use std::io::Read;

use crate::wuwa::consts::GameEdition;

use super::find_cdn_uri;

pub mod schema;

#[cached::proc_macro::cached(result)]
#[tracing::instrument(level = "trace")]
pub fn request(edition: GameEdition) -> anyhow::Result<schema::Response> {
    tracing::trace!("Fetching resource API");

    let cdn = find_cdn_uri(edition)?;
    let resources = super::game::request(edition)?.default.resources;

    let response = minreq::get(format!("{cdn}/{resources}"))
        .with_timeout(*crate::REQUESTS_TIMEOUT)
        .send()?;

    let json = match response.headers.get("content-encoding").map(String::as_str) {
        Some("gzip") => flate2::read::GzDecoder::new(response.as_bytes())
            .bytes()
            .collect::<Result<Vec<_>, _>>()?,

        Some("bz") => {
            let mut json = Vec::new();

            brotli_decompressor::brotli_decode(response.as_bytes(), &mut json);

            json
        },

        _ => response.as_bytes().to_vec()
    };

    Ok(serde_json::from_slice(&json)?)
}
