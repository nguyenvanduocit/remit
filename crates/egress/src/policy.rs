use std::collections::HashSet;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Why a destination written as text is not one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("{0:?} is not host:port")]
    NotHostPort(String),
    #[error("port {0:?} is not 1-65535")]
    Port(String),
    #[error("host is empty or longer than a name can be")]
    HostLength,
    #[error("address {0:?} names a zone")]
    Zone(String),
    #[error("host {0:?} holds a character a name does not")]
    HostCharacter(String),
}

/// A destination as the gate compares it: a canonical host (a lower-case name, or an address in its one canonical
/// spelling) and a port. Allowlist entries, CONNECT authorities and absolute URIs are all parsed into it, so one spelling
/// of a host cannot slip past the comparison of another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) host: String,
    pub(crate) port: u16,
    /// Set when host is an address literal.
    pub(crate) ip: Option<IpAddr>,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&join_host_port(&self.host, &self.port.to_string()))
    }
}

/// `host:port`, with a host that holds a colon (an IPv6 address) bracketed.
pub(crate) fn join_host_port(host: &str, port: &str) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Takes `host:port` or `[host]:port` apart; a missing port, a stray bracket or a bare IPv6 address is refused.
pub(crate) fn split_host_port(hostport: &str) -> Option<(&str, &str)> {
    let (head, port) = hostport.rsplit_once(':')?;
    if let Some(bracketed) = head.strip_prefix('[') {
        // The first ']' closes the bracket right before the last ':', and no other bracket follows the opening one.
        let host = bracketed.strip_suffix(']')?;
        if host.contains(['[', ']']) || port.contains(['[', ']']) {
            return None;
        }
        Some((host, port))
    } else {
        if head.contains(':') || hostport.contains(['[', ']']) {
            return None;
        }
        Some((head, port))
    }
}

/// Reads "host:port" (a bracketed IPv6 literal included).
pub(crate) fn parse_target(hostport: &str) -> Result<Target, TargetError> {
    let (host, port) = split_host_port(hostport).ok_or_else(|| TargetError::NotHostPort(hostport.to_string()))?;
    new_target(host, port)
}

/// Parses a host and a port written as text.
pub(crate) fn new_target(host: &str, port: &str) -> Result<Target, TargetError> {
    // Digits only: u16's parser takes a leading '+', which a port may not have.
    let n = (!port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
        .then(|| port.parse::<u16>().ok())
        .flatten()
        .filter(|n| *n != 0)
        .ok_or_else(|| TargetError::Port(port.to_string()))?;
    let (host, ip) = canonical_host(host)?;
    Ok(Target { host, port: n, ip })
}

/// Lower-cases a name and drops one trailing dot; an address literal becomes its one canonical spelling (an IPv4 address
/// written as IPv6, ::ffff:127.0.0.1, is the IPv4 address). A name holds letters, digits, hyphens, underscores and dots
/// only: an internationalized name is compared in its punycode form, so a look-alike never equals the name that was
/// allowed.
pub(crate) fn canonical_host(host: &str) -> Result<(String, Option<IpAddr>), TargetError> {
    let lower = host.to_lowercase();
    let host = lower.strip_suffix('.').unwrap_or(&lower);
    if host.is_empty() || host.len() > 253 {
        return Err(TargetError::HostLength);
    }
    if let Ok(a) = host.parse::<IpAddr>() {
        let a = a.to_canonical();
        return Ok((a.to_string(), Some(a)));
    }
    // An IPv6 address with a zone is refused as such; std does not parse zones at all.
    if let Some((addr, _)) = host.split_once('%')
        && addr.parse::<Ipv6Addr>().is_ok()
    {
        return Err(TargetError::Zone(host.to_string()));
    }
    if !host
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(TargetError::HostCharacter(host.to_string()));
    }
    Ok((host.to_string(), None))
}

/// Why the caller's allowlist or methods are not a policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("egress: the allowlist is empty: a gate that allows nothing is not started")]
    EmptyAllowlist,
    #[error("egress: allowlist entry: {0}")]
    Entry(#[source] TargetError),
    #[error("egress: method {0:?} is not an upper-case HTTP method")]
    Method(String),
}

/// What the caller fixed at construction: where a program may connect, and which methods a plain HTTP request may use.
/// It decides on names and ports alone; the address a name resolves to is judged by `routable`.
#[derive(Debug, Clone)]
pub(crate) struct Policy {
    allow: HashSet<String>,
    /// None allows every method.
    methods: Option<HashSet<String>>,
}

impl Policy {
    pub(crate) fn new(allow: &[String], methods: &[String]) -> Result<Policy, PolicyError> {
        if allow.is_empty() {
            return Err(PolicyError::EmptyAllowlist);
        }
        let mut p = Policy {
            allow: HashSet::new(),
            methods: None,
        };
        for entry in allow {
            p.allow
                .insert(parse_target(entry).map_err(PolicyError::Entry)?.to_string());
        }
        if !methods.is_empty() {
            let mut set = HashSet::new();
            for m in methods {
                if m.is_empty() || m.to_uppercase() != *m || m.contains([' ', '\t', '\r', '\n']) {
                    return Err(PolicyError::Method(m.clone()));
                }
                set.insert(m.clone());
            }
            p.methods = Some(set);
        }
        Ok(p)
    }

    pub(crate) fn allows(&self, t: &Target) -> bool {
        self.allow.contains(&t.to_string())
    }

    /// Says whether a plain HTTP request may use the method; no restriction was set when methods is None.
    pub(crate) fn allows_method(&self, m: &str) -> bool {
        self.methods.as_ref().is_none_or(|set| set.contains(m))
    }
}

/// Says whether an address a name resolved to is one a program may be sent to: a global unicast address, not the
/// machine itself, its network, a link-local address (the cloud metadata service lives there), or a range that reaches
/// one of those through a translation.
pub(crate) fn routable(a: IpAddr) -> bool {
    match a.to_canonical() {
        IpAddr::V4(v4) => routable_v4(v4),
        IpAddr::V6(v6) => routable_v6(v6),
    }
}

fn routable_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    let global_unicast = !(a.is_broadcast()
        || a.is_unspecified()
        || o[0] == 127
        || o[0] >= 224 && o[0] < 240
        || (o[0] == 169 && o[1] == 254));
    let private = o[0] == 10 || (o[0] == 172 && o[1] & 0xf0 == 16) || (o[0] == 192 && o[1] == 168);
    // Reserved ranges netip does not name: 0.0.0.0/8, 100.64.0.0/10, 192.0.0.0/24, 198.18.0.0/15, 240.0.0.0/4.
    let reserved = o[0] == 0
        || (o[0] == 100 && o[1] & 0xc0 == 64)
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && o[1] & 0xfe == 18)
        || o[0] & 0xf0 == 240;
    global_unicast && !private && !reserved
}

fn routable_v6(a: Ipv6Addr) -> bool {
    let s = a.segments();
    let global_unicast = !(a.is_unspecified() || a.is_loopback() || s[0] & 0xff00 == 0xff00 || s[0] & 0xffc0 == 0xfe80);
    let private = s[0] & 0xfe00 == 0xfc00;
    // 100::/64 (discard) and 2001::/32 (Teredo, which hides its IPv4 address obfuscated, so it is refused whole).
    let reserved = (s[0] == 0x100 && s[1] == 0 && s[2] == 0 && s[3] == 0) || (s[0] == 0x2001 && s[1] == 0);
    if !global_unicast || private || reserved {
        return false;
    }
    embedded_v4(a).is_none_or(routable_v4)
}

/// The IPv4 address that NAT64 (64:ff9b::/96) and 6to4 (2002::/16) carry inside an IPv6 one: the address they send the
/// traffic to, which must be as routable as any other.
fn embedded_v4(a: Ipv6Addr) -> Option<Ipv4Addr> {
    let b = a.octets();
    if b[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0] {
        return Some(Ipv4Addr::new(b[12], b[13], b[14], b[15]));
    }
    if b[0] == 0x20 && b[1] == 0x02 {
        return Some(Ipv4Addr::new(b[2], b[3], b[4], b[5]));
    }
    None
}
