pub trait ApiExt {
    type Edition;
    type Schema;
    type Error;

    /// Cache API fetching result
    fn cache_schema(edition: Self::Edition, result: Result<Self::Schema, Self::Error>);

    /// Retrieve cached API fetching result
    fn retrieve_schema_cache<'a>(edition: Self::Edition) -> Option<&'a Result<Self::Schema, Self::Error>>;

    /// Get API URI
    fn uri(edition: Self::Edition) -> &'static str;

    /// Fetch schema from the API
    fn fetch<'a>(edition: Self::Edition) -> &'a Result<Self::Schema, Self::Error>;
}
