use super::consts::GameEdition;

/// Check whether telemetry servers disabled
/// 
/// If some of them is not disabled, then this function will return its address
/// 
/// ```
/// use anime_game_core::star_rail::telemetry;
/// use anime_game_core::star_rail::consts::GameEdition;
/// 
/// if let Ok(None) = telemetry::is_disabled(GameEdition::Global) {
///     println!("Telemetry is disabled");
/// }
/// ```
#[tracing::instrument(level = "debug")]
pub fn is_disabled(game_edition: GameEdition) -> anyhow::Result<Option<String>> {
    tracing::debug!("Checking telemetry servers status");

    for server in game_edition.telemetry_servers() {
        if crate::check_domain::available(server)? {
            tracing::warn!("Server is not disabled: {server}");

            return Ok(Some(server.to_string()));
        }
    }

    Ok(None)
}
