use crate::wuwa::consts::GameEdition;

use super::find_cdn_uri;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching resource API");

    let cdn = find_cdn_uri(edition)?;
    let resources = super::game::request(edition)?.default.resources;

    Ok(minreq::get(format!("{cdn}/{resources}"))
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
