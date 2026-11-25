use std::sync::Arc;
use std::sync::atomic::Ordering;

use bytes::Bytes;
use http::header::{self, HeaderMap, HeaderValue};
use http::{Method, Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;

use crate::denied::showable;
use crate::egress::{Body, Inner, dial, full, text};
use crate::forward::strip_hop_by_hop;
use crate::inject::{Destination, Injection};
use crate::inject_rules::{forwardable, same_authority, split_request_uri};

/// The headers of the upstream's answer a program never receives: a cookie or a challenge could carry the credential or
/// ask for one, and the length and encoding are the injector's to state again.
const ANSWER_WITHHELD: &[&str] = &[
    "set-cookie",
    "set-cookie2",
    "www-authenticate",
    "authorization",
    "content-length",
    "content-encoding",
];

/// A request the injector will not send, and the status the program is answered with.
pub(crate) type Refusal = (StatusCode, String);

impl Inner {
    /// One request of a program to destination: judge it, ask the upstream with its credential added, and give back what
    /// the upstream said, with the credential removed and a redirect to another host replaced.
    pub(crate) async fn exchange(self: Arc<Self>, destination: &Destination, req: Request<Incoming>) -> Response<Body> {
        let Some(inj) = self.inject.as_ref() else {
            return text(
                StatusCode::INTERNAL_SERVER_ERROR,
                "egress gate: the gate does not inject\n",
            );
        };
        let label = showable(&format!("{} {}", req.method(), req.uri()));
        match self.ask(inj, destination, req).await {
            Ok(resp) => self.pass_on(inj, destination, &label, resp).await,
            Err((status, reason)) => self.refuse(&label, status, &reason),
        }
    }

    /// Judges the request and asks the upstream; the answer as the upstream gave it.
    async fn ask(
        &self,
        inj: &Injection,
        d: &Destination,
        req: Request<Incoming>,
    ) -> Result<Response<Incoming>, Refusal> {
        // The count is the run's, every destination's requests together. This request is number (the count before it) + 1:
        // over the cap when the count before it already reached it.
        if inj.requests.fetch_add(1, Ordering::SeqCst) >= inj.max_requests {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!("more than {} requests in one run", inj.max_requests),
            ));
        }
        let target = judge_request(d, &req)?;
        let (parts, body) = req.into_parts();
        let body = take_body(inj, d, &parts.method, body).await?;
        let out = upstream_request(d, &parts.method, &parts.headers, &target, body, (inj.now)())
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("cannot build the request: {e}")))?;
        let addrs = self.route(&d.target).await?;
        let no_answer = || (StatusCode::BAD_GATEWAY, format!("{} did not answer", d.target));
        let tcp = dial(&addrs, d.target.port).await.map_err(|_| no_answer())?;
        d.open(self.tracker.clone(), tcp, out).await.map_err(|_| no_answer())
    }

    /// Gives the program the upstream's answer, held whole so that the credential can be removed from all of it.
    async fn pass_on(&self, inj: &Injection, d: &Destination, label: &str, resp: Response<Incoming>) -> Response<Body> {
        let (mut parts, body) = resp.into_parts();
        let limit = usize::try_from(d.max_answer).unwrap_or(usize::MAX);
        let data = match Limited::new(body, limit).collect().await {
            Ok(c) => c.to_bytes(),
            Err(e) if e.is::<http_body_util::LengthLimitError>() => {
                return self.decline(label, &format!("the answer is over {} bytes", d.max_answer));
            }
            Err(_) => {
                return self.refuse(
                    label,
                    StatusCode::BAD_GATEWAY,
                    "the answer ended before it was complete",
                );
            }
        };
        if !inj.budget.take(u64::try_from(data.len()).unwrap_or(u64::MAX)) {
            return self.decline(label, "the run's byte cap is reached");
        }
        if parts.headers.contains_key(header::CONTENT_ENCODING) {
            return self.decline(
                label,
                "the upstream answered in an encoding the proxy cannot read, so it cannot remove the credential from it",
            );
        }
        if let Some(host) = leaves_the_host(parts.status, &parts.headers, d) {
            return self.decline(
                label,
                &format!(
                    "the upstream answered {} and sent the program to {}: redirects to another host are not followed or passed on",
                    parts.status.as_u16(),
                    showable(&host)
                ),
            );
        }
        strip_hop_by_hop(&mut parts.headers);
        for name in ANSWER_WITHHELD {
            parts.headers.remove(*name);
        }
        let mut clean = HeaderMap::with_capacity(parts.headers.len());
        for (name, value) in &parts.headers {
            if let Ok(v) = HeaderValue::from_str(&(d.scrub)(&String::from_utf8_lossy(value.as_bytes()))) {
                clean.append(name, v);
            }
        }
        let body = (d.scrub)(&String::from_utf8_lossy(&data));
        clean.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
        parts.headers = clean;
        Response::from_parts(parts, full(Bytes::from(body)))
    }

    /// Answers 502 for an answer the injector will not pass on, and records it: unlike a name that does not resolve, it
    /// is the injector's decision, and the program's account of what was refused names it.
    fn decline(&self, label: &str, reason: &str) -> Response<Body> {
        self.denied.add(label, reason);
        text(StatusCode::BAD_GATEWAY, &format!("egress gate: {reason}\n"))
    }
}

/// Applies the destination's rules to a request as the program wrote it: the path and query for the upstream.
fn judge_request(d: &Destination, req: &Request<Incoming>) -> Result<String, Refusal> {
    let uri = req.uri().to_string();
    let t = split_request_uri(&uri).map_err(|why| (StatusCode::BAD_REQUEST, why.to_string()))?;
    let scheme = d.scheme();
    if !t.scheme.is_empty() && t.scheme != scheme {
        return Err((
            StatusCode::FORBIDDEN,
            format!("the request is for {}, the connection is {scheme}", t.scheme),
        ));
    }
    if t.scheme.is_empty() && !d.https {
        return Err((
            StatusCode::BAD_REQUEST,
            "this is a forward proxy: send an absolute URI".to_string(),
        ));
    }
    if !t.authority.is_empty() && !same_authority(t.authority, &d.target, d.default_port()) {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "the request is for {}, which is not the connection's host {}",
                showable(t.authority),
                d.target
            ),
        ));
    }
    // The request's host: the authority of an absolute URI, else the Host header.
    let host = match t.authority {
        "" => req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or(""),
        a => a,
    };
    if !same_authority(host, &d.target, d.default_port()) {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "the Host header {} is not the connection's host {}",
                showable(host),
                d.target
            ),
        ));
    }
    d.rules
        .judge(req.method().as_str(), &t.path, &t.query)
        .map_err(|why| (StatusCode::FORBIDDEN, why))
}

/// Reads the request body, up to the destination's cap, and spends it from the run's budget; a GET or HEAD carries none
/// to the upstream.
async fn take_body(inj: &Injection, d: &Destination, method: &Method, body: Incoming) -> Result<Bytes, Refusal> {
    if method == Method::GET || method == Method::HEAD {
        return Ok(Bytes::new());
    }
    let limit = usize::try_from(d.max_request_body).unwrap_or(usize::MAX);
    let body = match Limited::new(body, limit).collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) if e.is::<http_body_util::LengthLimitError>() => {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("the request body is over {} bytes", d.max_request_body),
            ));
        }
        Err(_) => return Err((StatusCode::BAD_REQUEST, "cannot read the request body".to_string())),
    };
    if !inj.budget.take(u64::try_from(body.len()).unwrap_or(u64::MAX)) {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "the run's byte cap is reached".to_string(),
        ));
    }
    Ok(body)
}

/// The request the upstream receives: the judged target under the connection's address, the few headers of the program
/// that pass, and the credential; now is the time a signer is given.
fn upstream_request(
    d: &Destination,
    method: &Method,
    program: &HeaderMap,
    target: &str,
    body: Bytes,
    now: std::time::SystemTime,
) -> Result<Request<Full<Bytes>>, String> {
    let url = d.upstream_url(target)?;
    let (mut parts, ()) = Request::builder()
        .method(method.clone())
        .uri(url)
        .body(())
        .map_err(|e| e.to_string())?
        .into_parts();
    for (name, value) in program {
        if forwardable(name.as_str()) {
            parts.headers.append(name, value.clone());
        }
    }
    for (name, value) in &d.headers {
        parts.headers.insert(name, value.clone());
    }
    parts.headers.insert(
        header::HOST,
        HeaderValue::from_str(&d.base_host).map_err(|e| e.to_string())?,
    );
    if let Some(sign) = &d.sign {
        sign(&mut parts, &body, now).map_err(|e| format!("the request could not be signed: {e}"))?;
    }
    // The signer saw the absolute URL; the upstream is asked in origin form.
    parts.uri = parts
        .uri
        .path_and_query()
        .map_or_else(|| Uri::from_static("/"), |pq| Uri::from(pq.clone()));
    Ok(Request::from_parts(parts, Full::new(body)))
}

/// The host a redirect sends the program to when it names an origin other than the connection's (another host, another
/// port, the other scheme). A relative Location, or one on the same origin, stays with the program: asking for it asks
/// the injector again, which decides again.
fn leaves_the_host(status: StatusCode, headers: &HeaderMap, d: &Destination) -> Option<String> {
    if !status.is_redirection() {
        return None;
    }
    let loc = headers.get(header::LOCATION)?;
    let Ok(loc) = loc.to_str() else {
        return Some("an address that does not parse".to_string());
    };
    let (scheme, host) = location_origin(loc, d.scheme())?;
    let port = if scheme == "http" { 80 } else { 443 };
    if same_authority(host, &d.target, port) && scheme == d.scheme() {
        return None;
    }
    Some(host.to_string())
}

/// The scheme and authority a Location names, resolved against the request's scheme as a client does; None for a
/// relative one (no authority), which stays on the connection's origin.
fn location_origin<'a>(loc: &'a str, base_scheme: &'a str) -> Option<(String, &'a str)> {
    let authority_of = |rest: &'a str| rest.find(['/', '?', '#']).map_or(rest, |end| &rest[..end]);
    if let Some(rest) = loc.strip_prefix("//") {
        return Some((base_scheme.to_string(), authority_of(rest)));
    }
    let (scheme, rest) = loc.split_once(':')?;
    if scheme.contains(['/', '?', '#']) || scheme.is_empty() {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    Some((scheme, rest.strip_prefix("//").map_or("", authority_of)))
}
