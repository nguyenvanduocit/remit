//! What a request through the injecting proxy must be, as pure functions of text: the method and target a program wrote
//! in, the path for the upstream or the reason it is refused out. Nothing here reads a connection, so the hostile inputs
//! are a table.

use crate::denied::showable;
use crate::policy::{Target, new_target, split_host_port};

const MAX_PATH_BYTES: usize = 2048;
const MAX_QUERY_BYTES: usize = 8192;

/// Why the caller's prefix or read endpoints are not rules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RulesError {
    #[error("the address's path {path:?}: {why}")]
    Prefix { path: String, why: String },
    #[error("read post path {path:?}: {why}")]
    ReadPost { path: String, why: String },
    #[error("read post path {0:?} ends with /: name the endpoint, its sub-paths follow")]
    ReadPostSlash(String),
}

/// What one connection admits: the path prefix every request is placed under, and either reads only (GET, and POST at the
/// endpoints where it only reads) or writes too, as the grant allows.
#[derive(Debug, Clone)]
pub(crate) struct RequestRules {
    prefix: String,
    read_posts: Vec<String>,
    writes: bool,
}

impl RequestRules {
    /// Checks what the caller fixed. prefix is the path of the connection's address ("" or "/grafana", a trailing slash
    /// is dropped); read_posts are the paths where POST is a read, each a rooted path of its own; writes admits every
    /// method a request to a resource may have.
    pub(crate) fn new(prefix: &str, read_posts: &[String], writes: bool) -> Result<RequestRules, RulesError> {
        let prefix = prefix.trim_end_matches('/');
        if !prefix.is_empty()
            && let Some(why) = check_path(prefix)
        {
            return Err(RulesError::Prefix {
                path: prefix.to_string(),
                why,
            });
        }
        for p in read_posts {
            if let Some(why) = check_path(p) {
                return Err(RulesError::ReadPost { path: p.clone(), why });
            }
            if p != "/" && p.ends_with('/') {
                return Err(RulesError::ReadPostSlash(p.clone()));
            }
        }
        Ok(RequestRules {
            prefix: prefix.to_string(),
            read_posts: read_posts.to_vec(),
            writes,
        })
    }

    /// The path and query the upstream is asked for, or why the request is refused. On a connection that writes, every
    /// method passes but CONNECT and TRACE, which ask the upstream for a tunnel or to echo the request (its credential
    /// included) and are never a program's. On one that reads, GET reads anywhere, POST reads only at an endpoint the
    /// source named (and below it), and every other method is refused, because the proxy cannot know what it does. The
    /// path and query rules hold either way.
    pub(crate) fn judge(&self, method: &str, path: &str, query: &str) -> Result<String, String> {
        if let Some(why) = check_path(path) {
            return Err(format!("the path {} {why}", showable(path)));
        }
        if let Some(why) = check_query(query) {
            return Err(format!("the query {why}"));
        }
        match method {
            "CONNECT" | "TRACE" => {
                return Err(format!("method {method} is never sent to an upstream"));
            }
            _ if self.writes => {}
            "GET" => {}
            "POST" if self.is_read_post(path) => {}
            "POST" => {
                return Err(format!(
                    "POST {} is not a read here: POST is allowed only at {}",
                    showable(path),
                    self.post_list()
                ));
            }
            _ => {
                return Err(format!(
                    "method {} is not allowed: this connection only reads; GET reads, and POST {}",
                    showable(method),
                    self.post_at()
                ));
            }
        }
        let mut target = format!("{}{path}", self.prefix);
        if !query.is_empty() {
            target.push('?');
            target.push_str(query);
        }
        Ok(target)
    }

    fn is_read_post(&self, path: &str) -> bool {
        self.read_posts
            .iter()
            .any(|p| path == p || path.strip_prefix(p.as_str()).is_some_and(|rest| rest.starts_with('/')))
    }

    fn post_list(&self) -> String {
        if self.read_posts.is_empty() {
            "nowhere".to_string()
        } else {
            self.read_posts.join(", ")
        }
    }

    fn post_at(&self) -> String {
        if self.read_posts.is_empty() {
            "reads nowhere".to_string()
        } else {
            format!("reads at {}", self.read_posts.join(", "))
        }
    }
}

/// Says whether a byte may appear in a path as written: unreserved characters, sub-delimiters but ';' (a path parameter
/// some servers read as part of the segment before it), and the escape and the separator.
fn path_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"-._~!$&'()*+,=:@%/".contains(&c)
}

/// None for a rooted path that names one resource the way it is written: no parent or current directory, no empty
/// segment, no escape that decodes to a separator, a dot, a percent or a control byte, no byte a path does not hold. A
/// server that decodes twice or normalises differently must find nothing to disagree about.
pub(crate) fn check_path(p: &str) -> Option<String> {
    let b = p.as_bytes();
    if b.first() != Some(&b'/') {
        return Some("does not start with /".to_string());
    }
    if b.len() > MAX_PATH_BYTES {
        return Some(format!("is longer than {MAX_PATH_BYTES} bytes"));
    }
    if p.contains("//") {
        return Some("has an empty segment (//)".to_string());
    }
    let mut rest = b;
    while let [c, tail @ ..] = rest {
        if !path_byte(*c) {
            return Some("holds a byte a path does not (a space, control, non-ASCII, backslash or ; byte)".to_string());
        }
        rest = tail;
        if *c == b'%' {
            let Some((d, after)) = unescape(tail) else {
                return Some("has an incomplete escape (% needs two hex digits)".to_string());
            };
            if d == b'/' || d == b'\\' || d == b'.' || d == b'%' || d < b' ' || d == 0x7f {
                return Some("holds an encoded slash, dot, backslash, percent or control byte".to_string());
            }
            rest = after;
        }
    }
    for seg in p.split('/') {
        match seg {
            ".." => return Some("names a parent directory (..)".to_string()),
            "." => return Some("names the current directory (.)".to_string()),
            _ => {}
        }
    }
    None
}

/// None for a query of printable ASCII with complete escapes. Its content is the upstream's to read; what is refused is
/// what could end the request line or smuggle a second one.
fn check_query(q: &str) -> Option<String> {
    let b = q.as_bytes();
    if b.len() > MAX_QUERY_BYTES {
        return Some(format!("is longer than {MAX_QUERY_BYTES} bytes"));
    }
    let mut rest = b;
    while let [c, tail @ ..] = rest {
        let c = *c;
        if c <= b' ' || c > b'~' || c == b'#' || c == b'\\' {
            return Some(
                "holds a byte a query does not (a space, control, non-ASCII, # or backslash byte)".to_string(),
            );
        }
        rest = tail;
        if c == b'%' {
            let Some((_, after)) = unescape(tail) else {
                return Some("has an incomplete escape (% needs two hex digits)".to_string());
            };
            rest = after;
        }
    }
    None
}

/// The byte an escape stands for, read from what follows its '%', and the bytes after the escape.
fn unescape(after_percent: &[u8]) -> Option<(u8, &[u8])> {
    let [hi, lo, rest @ ..] = after_percent else {
        return None;
    };
    let hi = char::from(*hi).to_digit(16)?;
    let lo = char::from(*lo).to_digit(16)?;
    Some((u8::try_from(hi << 4 | lo).ok()?, rest))
}

/// A request target taken apart: the scheme and authority it names (both empty for the usual /path form), its path and
/// its query, as written.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RequestTarget<'a> {
    pub(crate) scheme: &'a str,
    pub(crate) authority: &'a str,
    pub(crate) path: String,
    pub(crate) query: String,
}

/// Takes a request target apart. An absolute form (what a client sends a plain-HTTP proxy) must be http or https and
/// carry no login.
pub(crate) fn split_request_uri(uri: &str) -> Result<RequestTarget<'_>, &'static str> {
    let mut t = RequestTarget::default();
    let mut rest = uri.to_string();
    for (prefix, scheme) in [("http://", "http"), ("https://", "https")] {
        if uri.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix)) {
            t.scheme = scheme;
            let after = &uri[prefix.len()..];
            let end = after.find(['/', '?']).unwrap_or(after.len());
            t.authority = &after[..end];
            if t.authority.is_empty() || t.authority.contains(['@', ' ', '\t', '\r', '\n']) {
                return Err("the request target names no host, or holds a login");
            }
            rest = after[end..].to_string();
            if !rest.starts_with('/') {
                rest.insert(0, '/');
            }
            break;
        }
    }
    if t.authority.is_empty() && (!uri.starts_with('/') || uri.starts_with("//")) {
        return Err("the request target is not a path (or an http absolute URI)");
    }
    if rest.contains(['#', ' ']) {
        return Err("the request target holds a fragment or a space");
    }
    let (path, query) = rest.split_once('?').unwrap_or((rest.as_str(), ""));
    t.path = path.to_string();
    t.query = query.to_string();
    Ok(t)
}

/// Says whether authority (a Host header, or the host of an absolute URI) names the upstream: the same host in its one
/// spelling and the same port, default_port when none is written.
pub(crate) fn same_authority(authority: &str, upstream: &Target, default_port: u16) -> bool {
    if authority.is_empty() || authority.contains(['@', ' ', '\t', '\r', '\n', '%', '/', '\\']) {
        return false;
    }
    let default = default_port.to_string();
    let (host, port) = split_host_port(authority).unwrap_or((authority.trim_matches(['[', ']']), &default));
    new_target(host, port).is_ok_and(|t| t.to_string() == upstream.to_string())
}

/// Says whether a header of the program's request goes to the upstream. The list that does is short on purpose: a header
/// can authenticate (Authorization, Cookie, an auth proxy's X-WEBAUTH-USER), steer (a method override, a forwarded
/// address) or change the answer's encoding, and none of them is a program's to send.
pub(crate) fn forwardable(name: &str) -> bool {
    ["accept", "accept-language", "content-type", "user-agent"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
}

#[cfg(test)]
mod tests {
    use super::RequestRules;

    fn rules(writes: bool) -> RequestRules {
        RequestRules::new("/api", &["/search".to_string()], writes).unwrap()
    }

    #[test]
    fn a_read_connection_admits_reads_and_refuses_every_write() {
        let r = rules(false);
        assert_eq!(r.judge("GET", "/dash", "a=1"), Ok("/api/dash?a=1".to_string()));
        assert_eq!(r.judge("POST", "/search/q", ""), Ok("/api/search/q".to_string()));
        for (method, path) in [
            ("POST", "/items"),
            ("PUT", "/items/1"),
            ("PATCH", "/items/1"),
            ("DELETE", "/items/1"),
        ] {
            assert!(
                r.judge(method, path, "").is_err(),
                "{method} {path} passed a read connection"
            );
        }
    }

    #[test]
    fn a_write_connection_admits_every_method_on_a_resource() {
        let r = rules(true);
        for method in ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"] {
            assert_eq!(
                r.judge(method, "/items/1", ""),
                Ok("/api/items/1".to_string()),
                "{method}"
            );
        }
    }

    #[test]
    fn connect_and_trace_are_never_admitted() {
        for writes in [false, true] {
            for method in ["CONNECT", "TRACE"] {
                assert!(
                    rules(writes).judge(method, "/items", "").is_err(),
                    "{method}, writes {writes}"
                );
            }
        }
    }

    #[test]
    fn a_write_connection_still_refuses_a_path_that_names_something_else() {
        let r = rules(true);
        for path in [
            "/items/../admin",
            "/items/%2f..%2fadmin",
            "/items/%2e%2e",
            "/items//1",
            "items",
            "/a;b",
        ] {
            for method in ["PUT", "DELETE", "POST"] {
                assert!(r.judge(method, path, "").is_err(), "{method} {path} passed");
            }
        }
        assert!(r.judge("PUT", "/items", "a b").is_err(), "a query with a space passed");
    }
}
