//! A text put into a URL, and read back out of one. An outside id or an agent's value goes into a request's path or query
//! through [`path_segment`] or [`query_value`], so it never adds a segment or a parameter of its own, and into a
//! delimited list through [`component`]; [`unescape`] reads a `%XX` text back strictly.

/// `s` as one path segment: letters, digits, `-_.~` and `$&+:=@` stay, every other byte (`/` included) is `%XX`.
#[must_use]
pub fn path_segment(s: &str) -> String {
    escape(s, |b| matches!(b, b'$' | b'&' | b'+' | b':' | b'=' | b'@'), "%20")
}

/// `s` as a query value or name: letters, digits and `-_.~` stay, a space is `+`, every other byte is `%XX`.
#[must_use]
pub fn query_value(s: &str) -> String {
    escape(s, |_| false, "+")
}

/// `s` with only letters, digits and `-_.~` as they are, every other byte `%XX`: a value inside a list that `,`, `=` or
/// `;` delimits (a W3C baggage value, `OTEL_RESOURCE_ATTRIBUTES`).
#[must_use]
pub fn component(s: &str) -> String {
    escape(s, |_| false, "%20")
}

/// `s` with letters, digits, `-_.~` and the bytes `keep` names as they are, a space as `space`, every other byte `%XX`.
fn escape(s: &str, keep: fn(u8) -> bool, space: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') || keep(b) {
            out.push(char::from(b));
        } else if b == b' ' {
            out.push_str(space);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `s` with each `%XX` read as its byte; none when an escape is cut short or is not hex, or the bytes are not UTF-8. A `+`
/// stays a `+`: this reads a path segment.
#[must_use]
pub fn unescape(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut rest = s.bytes();
    while let Some(b) = rest.next() {
        if b == b'%' {
            let hi = char::from(rest.next()?).to_digit(16)?;
            let lo = char::from(rest.next()?).to_digit(16)?;
            bytes.push(u8::try_from(hi.checked_mul(16)?.checked_add(lo)?).ok()?);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).ok()
}
