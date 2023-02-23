use super::consts::API_URI;
use super::json_schemas::versions::Response as ApiResponse;

use crate::api;

#[cached::proc_macro::cached]
#[tracing::instrument(level = "trace")]
pub fn try_fetch() -> Result<api::Response, curl::Error> {
    tracing::trace!("Trying to fetch API response");

    unsafe {
        api::try_fetch(API_URI)
    }
}

#[tracing::instrument(level = "trace")]
pub fn try_fetch_json() -> anyhow::Result<ApiResponse> {
    tracing::trace!("Trying to decode API response");

    Ok(try_fetch()?.try_json()?)
}
