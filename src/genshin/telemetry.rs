use super::consts::GameEdition;

/// Check whether telemetry servers disabled
/// 
/// If some of them is not disabled, then this function will return its address
/// 
/// Timeout param is optional number of seconds
/// 
/// ```
/// use anime_game_core::genshin::telemetry;
/// 
/// // 3 seconds timeout
/// if let None = telemetry::is_disabled(Some(3)) {
///     println!("Telemetry is disabled");
/// }
/// ```
#[tracing::instrument(level = "debug")]
pub fn is_disabled(timeout: Option<u64>) -> Option<String> {
    tracing::debug!("Checking telemetry servers status");

    for server in GameEdition::selected().telemetry_servers() {
        let mut request = minreq::head(format!("http://{server}"));

        if let Some(timeout) = timeout {
            request = request.with_timeout(timeout);
        }

        if let Ok(_) = request.send() {
            tracing::warn!("Telemetry server is not disabled: {server}");

            return Some(server.to_string());
        }
    }

    None
}
