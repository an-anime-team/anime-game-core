pub mod schema;

use super::consts::API_URI;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request() -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API");

    Ok(minreq::get(API_URI)
        .with_timeout(crate::DEFAULT_REQUESTS_TIMEOUT)
        .send()?.json()?)
}
