/// Check whether given domain name is resolvable
/// 
/// Timeout is optional amount of seconds
#[tracing::instrument(level = "trace")]
pub fn is_disabled<T: AsRef<str> + std::fmt::Debug>(domain: T, timeout: Option<u64>) -> bool {
    let mut request = minreq::head(format!("http://{}", domain.as_ref()));

    if let Some(timeout) = timeout {
        request = request.with_timeout(timeout);
    }

    request.send().is_ok()
}
