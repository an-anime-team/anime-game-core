pub mod schema;

use crate::pgr::consts::API_BASE_URI;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request() -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching resource API");

    Ok(minreq::get(format!("{API_BASE_URI}/{}", super::game::request()?.default.resources))
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
