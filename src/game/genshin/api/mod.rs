use crate::network::api::ApiExt;

use super::Edition;

pub mod schema;

use schema::Response;

static mut GLOBAL_CACHE: Option<Result<Response, minreq::Error>> = None;
static mut CHINA_CACHE: Option<Result<Response, minreq::Error>> = None;

pub struct Api;

impl ApiExt for Api {
    type Edition = Edition;
    type Schema = Response;
    type Error = minreq::Error;

    fn cache_schema(edition: Self::Edition, result: Result<Self::Schema, Self::Error>) {
        unsafe {
            match edition {
                Edition::Global => GLOBAL_CACHE = Some(result),
                Edition::China  => CHINA_CACHE  = Some(result)
            }
        }
    }

    fn retrieve_schema_cache<'a>(edition: Self::Edition) -> Option<&'a Result<Self::Schema, Self::Error>> {
        unsafe {
            match edition {
                Edition::Global => GLOBAL_CACHE.as_ref(),
                Edition::China  => CHINA_CACHE.as_ref()
            }
        }
    }

    fn uri(edition: Self::Edition) -> &'static str {
        match edition {
            Edition::Global => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10"),
            Edition::China  => concat!("https://sdk-static.", "mih", "oyo", ".com/hk4e_cn/mdk/launcher/api/resource?key=eYd89JmJ&launcher_id=18")
        }
    }

    fn fetch<'a>(edition: Self::Edition) -> &'a Result<Self::Schema, Self::Error> {
        if let Some(result) = Self::retrieve_schema_cache(edition) {
            result
        }

        else {
            let result = minreq::get(Self::uri(edition)).send()
                .and_then(|result| result.json());

            Self::cache_schema(edition, result);

            // In practice it's impossible to get that error because
            // we literally store the cache a line above
            Self::retrieve_schema_cache(edition)
                .expect("Failed to cache API response")
        }
    }
}
