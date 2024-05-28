use crate::wuwa::consts::GameEdition;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching game API");

    Ok(minreq::get(edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
