pub mod schema;

use crate::honkai::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(game_edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API for {:?}", game_edition);

    Ok(minreq::get(game_edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
