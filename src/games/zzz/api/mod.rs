pub mod schema;

use crate::zzz::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(game_edition: GameEdition) -> anyhow::Result<schema::GamePackage> {
    tracing::trace!("Fetching API for {:?}", game_edition);

    let schema: schema::Response = minreq::get(game_edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?;

    schema.data.game_packages.into_iter()
        .find(|game| game.game.biz.starts_with("nap_"))
        .ok_or_else(|| anyhow::anyhow!("Failed to find the game in the API"))
}

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn branches(game_edition: GameEdition) -> anyhow::Result<schema::GameBranch> {
    tracing::trace!("Fetching branches API for {:?}", game_edition);

    let schema: schema::BranchesResponse = minreq::get(game_edition.branches_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?;

    schema.data.game_branches.into_iter()
        .find(|game| game.game.biz.starts_with("nap_"))
        .ok_or_else(|| anyhow::anyhow!("Failed to find the game in the branches API"))
}
