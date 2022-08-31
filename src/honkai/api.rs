use super::consts::API_URI;
use super::json_schemas::versions::Response as ApiResponse;

use crate::api;

#[cached::proc_macro::cached]
pub fn try_fetch() -> Result<api::Response, curl::Error> {
    unsafe {
        api::try_fetch(API_URI)
    }
}

pub fn try_fetch_json() -> anyhow::Result<ApiResponse> {
    Ok(try_fetch()?.try_json()?)
}
