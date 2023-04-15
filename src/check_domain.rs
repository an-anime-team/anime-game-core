/// Check whether given domain name is resolvable
/// 
/// Timeout is optional amount of seconds
#[tracing::instrument(level = "trace")]
pub fn available<T: AsRef<str> + std::fmt::Debug>(domain: T, timeout: Option<u64>) -> bool {
    let ips = dns_lookup::lookup_host(domain.as_ref()).unwrap();
    !ips.contains(&"0.0.0.0".parse().unwrap())
}
