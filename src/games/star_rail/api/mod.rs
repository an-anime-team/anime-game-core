pub mod schema;

use crate::star_rail::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ GameEdition::selected() }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API");

    Ok(minreq::get(GameEdition::selected().api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
