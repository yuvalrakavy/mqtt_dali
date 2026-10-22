//! Optional authenticated HTTP access, owned by the managed DALI service.
use sha2::{Digest, Sha256};
use std::net::SocketAddr;

mod auth;
mod handlers;
pub(crate) mod server;
mod transport;
pub(crate) use server::Server;

/// Validated startup configuration. Deliberately not Debug/Serialize: it owns a
/// password verifier. Construction opens no resources and enables no listener.
#[derive(Clone)]
pub struct Settings {
    bind: SocketAddr,
    origin: String,
    password: [u8; 32],
    secure: bool,
}
impl Settings {
    pub fn new(bind: SocketAddr, origin: &str, password: &str) -> Result<Self, String> {
        if password.is_empty() || password.len() > 1024 {
            return Err("web requires a nonempty password of at most 1024 bytes".into());
        }
        let uri: axum::http::Uri = origin.parse().map_err(|_| "invalid web origin")?;
        let scheme = uri.scheme_str().ok_or("web origin needs http or https")?;
        if !matches!(scheme, "http" | "https")
            || !matches!(uri.path(), "" | "/")
            || uri.query().is_some()
            || origin.contains('#')
        {
            return Err("web origin must be an http(s) origin without path/query/fragment".into());
        }
        let authority = uri.authority().ok_or("web origin needs a host")?;
        if authority.host().is_empty()
            || authority.as_str().contains(['@', '%'])
            || (!authority.as_str()[authority.host().len()..].is_empty()
                && authority.as_str()[authority.host().len()..]
                    .strip_prefix(':')
                    .and_then(|port| port.parse::<u16>().ok())
                    .is_none())
        {
            return Err("invalid web origin authority".into());
        }
        let host = authority.host().to_ascii_lowercase();
        let port = authority.port_u16();
        let origin = match port {
            None | Some(80) if scheme == "http" => format!("http://{host}"),
            None | Some(443) if scheme == "https" => format!("https://{host}"),
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => unreachable!("validated scheme"),
        };
        Ok(Self {
            bind,
            origin,
            password: digest(password),
            secure: scheme == "https",
        })
    }
}
fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

#[cfg(test)]
mod tests;
