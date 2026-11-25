//! The built browser app (`web/dist`, produced by `npm run build`), embedded in the binary: every file as it is, and
//! `index.html` for any other path, the app's own routes.

use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Dist;

/// The app's Content-Security-Policy: images come from the app itself, never from an address an agent wrote into the
/// Markdown it shows (the browser would call it as the owner).
const APP_IMAGES: &str = "img-src 'self' data: blob:";

/// The build of the app in the binary, as the build wrote it: the page sends the same name when it connects, so the
/// server tells a page left open from an older build. None when the binary holds no build.
pub fn bundle() -> Option<String> {
    let f = Dist::get("bundle.txt")?;
    let b = String::from_utf8_lossy(&f.data).trim().to_owned();
    (!b.is_empty()).then_some(b)
}

/// Serves the file uri names, or `index.html` for a path that names none.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (name, file) = match Dist::get(path).filter(|_| !path.is_empty()) {
        Some(f) => (path, f),
        None => match Dist::get("index.html") {
            Some(f) => ("index.html", f),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "this binary holds no web app: build it with `make build`\n",
                )
                    .into_response();
            }
        },
    };
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    let mut res = (StatusCode::OK, file.data.into_owned()).into_response();
    let h = res.headers_mut();
    h.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(APP_IMAGES));
    if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
        h.insert(CONTENT_TYPE, v);
    }
    res
}
