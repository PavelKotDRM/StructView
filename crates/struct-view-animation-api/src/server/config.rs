use super::TlsFiles;
use rustls::ServerConfig;
use std::io::BufReader;
use std::net::IpAddr;
use std::sync::Arc;

pub(super) fn host_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    }
}

pub(super) fn validate_host(host: &str) -> Result<String, String> {
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if ip.is_unspecified() {
            return Err("Public host must not be an unspecified IP".into());
        }
        return Ok(host_text(ip));
    }
    if host.len() > 253
        || host.is_empty()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err("Public host must be an IP or DNS name without scheme, port or path".into());
    }
    Ok(host.to_ascii_lowercase())
}

pub(super) fn load_tls(files: &TlsFiles) -> Result<ServerConfig, String> {
    let cert = std::fs::File::open(&files.certificate)
        .map_err(|error| format!("Cannot read TLS certificate: {error}"))?;
    let certificates = rustls_pemfile::certs(&mut BufReader::new(cert))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Invalid TLS certificate PEM: {error}"))?;
    if certificates.is_empty() {
        return Err("TLS certificate PEM contains no certificates".into());
    }
    let key = std::fs::File::open(&files.private_key)
        .map_err(|error| format!("Cannot read TLS private key: {error}"))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key))
        .map_err(|error| format!("Invalid TLS private key PEM: {error}"))?
        .ok_or("TLS PEM contains no private key")?;
    let mut config =
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|error| format!("Cannot configure TLS: {error}"))?
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .map_err(|error| format!("Invalid TLS certificate/key: {error}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}
