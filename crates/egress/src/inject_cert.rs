use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, CidrSubnet, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, KeyPair, KeyUsagePurpose, NameConstraints, PKCS_ECDSA_P256_SHA256,
    SanType, SerialNumber,
};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// How long the certificates of one injector are valid: a run lasts seconds, and nothing outlives it.
const CERT_LIFETIME: Duration = Duration::from_secs(2 * 60 * 60);

/// What the injector shows a program that connects: the certificate for the host and its key.
pub(crate) struct ServerIdentity {
    pub(crate) cert: CertificateDer<'static>,
    pub(crate) key: PrivateKeyDer<'static>,
    /// The authority that signed it, as PEM, for the program to trust.
    pub(crate) ca_pem: Vec<u8>,
}

/// Makes what the injector shows a program that connects to host (a name, or an address literal): a certificate for
/// that host alone, and the authority that signed it as PEM, for the program to trust.
///
/// The authority is made for this injector, signs the one certificate, and its private key is dropped before the
/// function returns: nothing keeps it, so nothing can be made to sign another host. The authority is also
/// name-constrained to host, so a program that trusts it (and only it) trusts it for that host and no other. What the
/// program receives is a certificate and no key.
pub(crate) fn server_identity(host: &str, now: SystemTime) -> Result<ServerIdentity, String> {
    let fail = |what: &str, e: rcgen::Error| format!("{what}: {e}");
    let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|e| fail("authority key", e))?;
    let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|e| fail("server key", e))?;
    let not_before = now
        .checked_sub(Duration::from_secs(60))
        .ok_or("the clock is before the certificate's start can be written")?;
    let not_after = now
        .checked_add(CERT_LIFETIME)
        .ok_or("the clock is past where the certificate's end can be written")?;
    let not_before = time::OffsetDateTime::from(not_before);
    let not_after = time::OffsetDateTime::from(not_after);
    let ip: Option<IpAddr> = host.parse().ok();

    let mut ca = CertificateParams::default();
    ca.serial_number = Some(serial()?);
    ca.distinguished_name = name("Remit script proxy (one run)");
    ca.not_before = not_before;
    ca.not_after = not_after;
    ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca.key_usages = vec![KeyUsagePurpose::KeyCertSign];

    let mut leaf = CertificateParams::default();
    leaf.serial_number = Some(serial()?);
    leaf.distinguished_name = name(host);
    leaf.not_before = not_before;
    leaf.not_after = not_after;
    leaf.is_ca = IsCa::ExplicitNoCa;
    leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let permitted = match ip {
        Some(ip) => {
            leaf.subject_alt_names = vec![SanType::IpAddress(ip)];
            let bits = if ip.is_ipv4() { 32 } else { 128 };
            GeneralSubtree::IpAddress(CidrSubnet::from_addr_prefix(ip, bits))
        }
        None => {
            let dns = host.try_into().map_err(|e| fail("server certificate", e))?;
            leaf.subject_alt_names = vec![SanType::DnsName(dns)];
            GeneralSubtree::DnsName(host.to_string())
        }
    };
    ca.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![permitted],
        excluded_subtrees: Vec::new(),
    });

    let issuer = CertifiedIssuer::self_signed(ca, ca_key).map_err(|e| fail("authority certificate", e))?;
    let cert = leaf
        .signed_by(&leaf_key, &issuer)
        .map_err(|e| fail("server certificate", e))?;
    Ok(ServerIdentity {
        cert: cert.der().clone(),
        key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
        ca_pem: issuer.pem().into_bytes(),
    })
}

fn name(common: &str) -> DistinguishedName {
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common);
    dn
}

/// A random certificate serial number below 2^126.
fn serial() -> Result<SerialNumber, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| format!("the system's random source failed: {e}"))?;
    b[0] &= 0x3f;
    Ok(SerialNumber::from_slice(&b))
}
