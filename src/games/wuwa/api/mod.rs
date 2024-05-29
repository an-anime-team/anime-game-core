use crate::wuwa::consts::GameEdition;

pub mod game;
pub mod resource;

#[tracing::instrument]
#[cached::proc_macro::cached(result)]
/// Find CDN link from the API response
pub fn find_cdn_uri(edition: GameEdition) -> anyhow::Result<String> {
    tracing::trace!("Finding CDN address");

    let api = game::request(edition)?.default;

    let cdn = api.cdnList.iter()
        .min_by(|a, b| a.P.cmp(&b.P));

    let Some(cdn) = cdn else {
        anyhow::bail!("Failed to find game CDN link");
    };

    Ok(cdn.url.strip_suffix('/').unwrap().to_string())
}
