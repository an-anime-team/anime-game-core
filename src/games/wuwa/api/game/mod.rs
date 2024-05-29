use std::io::Read;

use crate::wuwa::consts::GameEdition;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching game API");

    let response = minreq::get(edition.api_uri())
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?;

    let json = flate2::read::GzDecoder::new(response.as_bytes())
        .bytes()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::from_slice(&json)?)
}
