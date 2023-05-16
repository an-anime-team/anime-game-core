pub mod schema;

use crate::genshin::consts::GameEdition;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request() -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching API");

    Ok(minreq::get(GameEdition::selected().api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?.json()?)
}
