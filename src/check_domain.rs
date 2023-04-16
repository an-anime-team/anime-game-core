/// Check whether given domain name is resolvable
/// 
/// Timeout is optional amount of seconds
#[tracing::instrument(level = "trace")]
pub fn available<T: AsRef<str> + std::fmt::Debug>(domain: T) -> anyhow::Result<bool> {
    for ip in dns_lookup::lookup_host(domain.as_ref())? {
        if !ip.is_loopback() && !ip.is_unspecified() {
            return Ok(true);
        }
    }

    Ok(false)
}
