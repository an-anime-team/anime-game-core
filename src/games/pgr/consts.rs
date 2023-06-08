pub const API_BASE_URI: &str = "https://prod-awscdn-gamestarter.kurogame.net";
pub const API_DATA_URI: &str = "https://prod-awscdn-gamestarter.kurogame.net/pcstarter/prod/game/G143/4/index.json";

/// Name of the game's data folder
pub const DATA_FOLDER_NAME: &str = "PGR_Data";

/// List of game telemetry servers
pub const TELEMETRY_SERVERS: &[&str] = &[
    // TODO
];

/// Default amount of threads `VersionDiff` will use to download stuff
pub const DEFAULT_DOWNLOADER_THREADS: usize = 8;
