use super::consts::TELEMETRY_SERVERS;

/// Check whether telemetry servers are disabled
#[tracing::instrument(level = "debug")]
pub fn is_disabled(timeout: Option<u64>) -> Option<String> {
    tracing::debug!("Checking telemetry servers status");

    for server in TELEMETRY_SERVERS {
        if !crate::check_domain::is_disabled(server, timeout) {
            tracing::warn!("Server is not disabled: {server}");

            return Some(server.to_string());
        }
    }

    None
}
