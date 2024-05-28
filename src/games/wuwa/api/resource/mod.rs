use crate::wuwa::consts::GameEdition;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching resource API");

    Ok(minreq::get(format!("{}/{}", edition.cdn_uri(), super::game::request(edition)?.default.resources))
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
