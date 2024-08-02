pub mod schema;

mod schema_old;

use crate::honkai::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(game_edition: GameEdition) -> anyhow::Result<schema::GamePackage> {
    tracing::trace!("Fetching API for {:?}", game_edition);

    let response = minreq::get(game_edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?;

    let schema = match game_edition {
        // New API
        GameEdition::China => response.json::<schema::Response>()?,

        // Old API
        _ => {
            let response = response.json::<schema_old::Response>()?;

            schema::Response::from(response)
        }
    };

    schema.data.game_packages.into_iter()
        .find(|game| game.game.biz.starts_with("bh3_"))
        .ok_or_else(|| anyhow::anyhow!("Failed to find the game in the API"))
}
