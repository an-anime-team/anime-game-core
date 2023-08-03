pub mod schema;

use crate::genshin::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(game_edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API for {:?}", game_edition);

    let mut schema: schema::Response = minreq::get(game_edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?;

    // FIXME: temporary workaround! Fix this later!!!!
    if schema.data.game.latest.path.is_empty() {
        let url = &schema.data.game.latest.segments[0].path;

        schema.data.game.latest.path = url[..url.len() - 4].to_string();
    }

    Ok(schema)
}
