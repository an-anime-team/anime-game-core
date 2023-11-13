pub mod schema;

use crate::star_rail::consts::GameEdition;

#[cached::proc_macro::cached(
    key = "GameEdition",
    convert = r#"{ game_edition }"#,
    result
)]
#[tracing::instrument(level = "trace")]
pub fn request(game_edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API for {:?}", game_edition);

    let mut response = minreq::get(game_edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json::<schema::Response>()?;

    // FIXME: temporary workaround for 1.5.0 version
    if let Some(predownload) = &mut response.data.pre_download_game {
        response.data.game.latest.voice_packs = predownload.latest.voice_packs.clone();

        for diff in &mut response.data.game.diffs {
            diff.voice_packs = predownload.latest.voice_packs.clone();
        }

        for diff in &mut predownload.diffs {
            diff.voice_packs = predownload.latest.voice_packs.clone();
        }
    }

    Ok(response)
}
