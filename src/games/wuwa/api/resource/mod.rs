use crate::wuwa::consts::GameEdition;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching resource API");

    let api = super::game::request(edition)?.default;

    let cdn = api.cdnList.iter()
        .min_by(|a, b| a.P.cmp(&b.P));

    let Some(cdn) = cdn else {
        anyhow::bail!("Failed to find game CDN link");
    };

    Ok(minreq::get(format!("{}/{}", cdn.url, api.resources))
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
