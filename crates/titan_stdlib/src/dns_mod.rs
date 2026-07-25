//! Pure-Rust DNS resolution (`std::dns::*`) via `hickory-resolver`.
//!
//! Blocking wrappers exposed to `.titan`; internally we use a private
//! Tokio single-thread runtime so no async plumbing leaks to the VM.

use std::net::IpAddr;

use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use once_cell::sync::OnceCell;
use thiserror::Error;
use tokio::runtime::Runtime;

#[derive(Debug, Error)]
pub enum DnsError {
    #[error("DNS runtime error: {0}")]
    Runtime(String),
    #[error("DNS lookup error: {0}")]
    Lookup(String),
}

fn runtime() -> Result<&'static Runtime, DnsError> {
    static RT: OnceCell<Runtime> = OnceCell::new();
    RT.get_or_try_init(|| Runtime::new().map_err(|e| DnsError::Runtime(e.to_string())))
}

fn resolver() -> Result<&'static TokioAsyncResolver, DnsError> {
    static R: OnceCell<TokioAsyncResolver> = OnceCell::new();
    R.get_or_try_init(|| {
        // Try system config first (/etc/resolv.conf on Android/Termux),
        // fall back to the well-known Google/Cloudflare servers.
        let (config, opts) = hickory_resolver::system_conf::read_system_conf()
            .unwrap_or_else(|_| (ResolverConfig::default(), ResolverOpts::default()));
        Ok(TokioAsyncResolver::tokio(config, opts))
    })
}

/// Resolves a hostname to a list of IP addresses (both v4 and v6).
pub fn resolve(host: &str) -> Result<Vec<String>, DnsError> {
    let rt = runtime()?;
    let r = resolver()?;
    let host = host.to_string();
    rt.block_on(async move {
        let response = r.lookup_ip(host).await.map_err(|e| DnsError::Lookup(e.to_string()))?;
        Ok(response.iter().map(|ip: IpAddr| ip.to_string()).collect())
    })
}

/// Resolves a hostname to IPv4 addresses only.
pub fn resolve_ipv4(host: &str) -> Result<Vec<String>, DnsError> {
    Ok(resolve(host)?.into_iter().filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok()).collect())
}

/// Resolves a hostname to IPv6 addresses only.
pub fn resolve_ipv6(host: &str) -> Result<Vec<String>, DnsError> {
    Ok(resolve(host)?.into_iter().filter(|ip| ip.parse::<std::net::Ipv6Addr>().is_ok()).collect())
}

/// Looks up MX records (returns `["priority host", ...]`, sorted).
pub fn resolve_mx(host: &str) -> Result<Vec<String>, DnsError> {
    let rt = runtime()?;
    let r = resolver()?;
    let host = host.to_string();
    rt.block_on(async move {
        let response = r.mx_lookup(host).await.map_err(|e| DnsError::Lookup(e.to_string()))?;
        let mut records: Vec<_> = response.iter().map(|mx| format!("{} {}", mx.preference(), mx.exchange().to_utf8())).collect();
        records.sort();
        Ok(records)
    })
}

/// Looks up TXT records, joining their fragments.
pub fn resolve_txt(host: &str) -> Result<Vec<String>, DnsError> {
    let rt = runtime()?;
    let r = resolver()?;
    let host = host.to_string();
    rt.block_on(async move {
        let response = r.txt_lookup(host).await.map_err(|e| DnsError::Lookup(e.to_string()))?;
        Ok(response.iter().map(|record| {
            record.txt_data().iter()
                .map(|piece| String::from_utf8_lossy(piece).into_owned())
                .collect::<String>()
        }).collect())
    })
}

/// Looks up CNAME record if present.
pub fn resolve_cname(host: &str) -> Result<Vec<String>, DnsError> {
    let rt = runtime()?;
    let r = resolver()?;
    let host = host.to_string();
    rt.block_on(async move {
        // A generic lookup for CNAME:
        let response = r
            .lookup(host, hickory_resolver::proto::rr::RecordType::CNAME)
            .await
            .map_err(|e| DnsError::Lookup(e.to_string()))?;
        Ok(response.iter().filter_map(|record| record.as_cname().map(|n| n.to_utf8())).collect())
    })
}

/// Reverse DNS (`.in-addr.arpa` / `.ip6.arpa`): returns hostnames for a given IP.
pub fn reverse(ip: &str) -> Result<Vec<String>, DnsError> {
    let rt = runtime()?;
    let r = resolver()?;
    let addr: IpAddr = ip.parse().map_err(|_| DnsError::Lookup(format!("invalid IP: {ip}")))?;
    rt.block_on(async move {
        let response = r.reverse_lookup(addr).await.map_err(|e| DnsError::Lookup(e.to_string()))?;
        Ok(response.iter().map(|name| name.to_utf8()).collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live DNS tests are opt-in with TITAN_DNS_LIVE=1 (network required).
    #[test]
    fn live_resolve_when_enabled() {
        if std::env::var("TITAN_DNS_LIVE").is_err() { return; }
        let ips = resolve("one.one.one.one").unwrap();
        assert!(ips.iter().any(|ip| ip == "1.1.1.1"), "one.one.one.one must resolve to 1.1.1.1, got {ips:?}");
    }

    #[test]
    fn reverse_rejects_invalid_ip() {
        assert!(reverse("not an ip").is_err());
    }
}
