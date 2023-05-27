pub mod schema;

use crate::pgr::consts::API_DATA_URI;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request() -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching game API");

    Ok(minreq::get(API_DATA_URI)
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
