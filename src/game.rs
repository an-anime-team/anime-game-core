// SPDX-License-Identifier: GPL-3.0-or-later
//
// anime-game-core
// Copyright (C) 2026  Nikita Podvirnyi <krypt0nn@dawn.wine>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::sync::Arc;
use std::path::PathBuf;

use sophon_tools::export::tokio::runtime::Handle as TokioHandle;
use sophon_tools::region::SophonRegion;
use sophon_tools::api::SophonApi;
use sophon_tools::api::responses::GameVersionInfo;
use sophon_tools::downloader::{
    SophonDownloader,
    SophonDownloaderProgressMsg,
    SophonDownloaderVerifyMethod
};
use sophon_tools::updater::{
    SophonUpdater,
    SophonUpdaterProgressMsg,
    SophonUpdaterVerifyMethod
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameVariant {
    ZenlessZoneZero
}

impl GameVariant {
    /// Get current sophon `game_biz`.
    #[inline]
    pub const fn sophon_game_biz(&self, region: SophonRegion) -> &'static str {
        match (self, region) {
            (Self::ZenlessZoneZero, SophonRegion::Global) => "nap_global",
            (Self::ZenlessZoneZero, SophonRegion::China)  => "nap_cn"
        }
    }

    /// Try to find sophon `game_id` for current game variant.
    pub async fn find_sophon_game_id(
        &self,
        api: &SophonApi,
        region: SophonRegion
    ) -> anyhow::Result<Option<String>> {
        let game_biz = self.sophon_game_biz(region);

        let game_id = api.fetch_games_branches_info(region, None).await?
            .iter()
            .find(|config| config.game.game_biz == game_biz)
            .map(|config| config.game.game_id.clone());

        Ok(game_id)
    }
}

pub struct Game {
    api: SophonApi,
    region: SophonRegion,
    game_id: String,
    path: PathBuf
}

impl Game {
    /// Create new game struct with given game variant and installation path.
    pub async fn new(
        variant: GameVariant,
        region: SophonRegion,
        path: PathBuf
    ) -> anyhow::Result<Self> {
        let api = SophonApi::default();

        let Some(game_id) = variant.find_sophon_game_id(&api, region)
            .await?
        else {
            anyhow::bail!("failed to find game id");
        };

        Ok(Self {
            api,
            region,
            game_id,
            path
        })
    }

    /// Detect game version from current installation directory.
    pub async fn detect_version(&self) -> anyhow::Result<Option<String>> {
        Ok(self.api.game(self.region, None, self.game_id.clone())
            .detect_version(&self.path).await?)
    }

    /// Fetch list of available game versions.
    pub async fn fetch_versions(&self) -> anyhow::Result<Box<[GameVersionInfo]>> {
        Ok(self.api.game(self.region, None, self.game_id.clone())
            .fetch_versions_info().await?.versions)
    }

    /// Detect current game version, fetch list of available game updates, and
    /// determine game installation diff (whether update is needed).
    pub async fn diff<'game>(
        &'game self
    ) -> anyhow::Result<Option<GameDiff<'game>>> {
        let Some(latest_version) = self.fetch_versions().await?
            .into_iter()
            .find(|version| !version.is_predownload)
            .map(|version| version.version)
        else {
            return Ok(None);
        };

        let Some(curr_version) = self.detect_version().await? else {
            return Ok(Some(GameDiff::Download {
                game: self,
                version: latest_version
            }));
        };

        if curr_version == latest_version {
            return Ok(None);
        }

        if self.api.game(self.region, None, self.game_id.clone())
            .updatable_versions().await?
            .into_iter()
            .any(|version| version == curr_version)
        {
            Ok(Some(GameDiff::Update {
                game: self,
                from_version: curr_version,
                to_version: latest_version
            }))
        } else {
            Ok(Some(GameDiff::Download {
                game: self,
                version: latest_version
            }))
        }
    }

    /// Get game download diff for currently installed (or latest available)
    /// game version. Functionally identical to repairing game installation.
    pub async fn repair<'game>(&'game self) -> anyhow::Result<GameDiff<'game>> {
        let Some(curr_version) = self.detect_version().await? else {
            let Some(latest_version) = self.fetch_versions().await?
                .into_iter()
                .find(|version| !version.is_predownload)
                .map(|version| version.version)
            else {
                anyhow::bail!("failed to find latest game version");
            };

            return Ok(GameDiff::Download {
                game: self,
                version: latest_version
            });
        };

        Ok(GameDiff::Download {
            game: self,
            version: curr_version
        })
    }
}

// TODO: predownloads support

#[derive(Clone)]
pub enum GameDiff<'game> {
    /// Download given game version.
    Download {
        game: &'game Game,
        version: String
    },

    /// Update from one game version to another.
    Update {
        game: &'game Game,
        from_version: String,
        to_version: String
    }
}

impl<'game> GameDiff<'game> {
    /// Apply game diff.
    pub async fn apply(&self, options: GameDiffOptions) -> anyhow::Result<()> {
        // Find game component download info.
        let component = options.component.as_deref()
            .unwrap_or("game");

        match self {
            Self::Download { game, version } => {
                let Some(download_manifest) = game.api.game(game.region, None, game.game_id.clone())
                    .package(Some(version.clone())).await?
                    .find_download_manifest(component).await?
                else {
                    anyhow::bail!("failed to find package download manifest");
                };

                // Prepare sophon downloader.
                let mut downloader = SophonDownloader::default();

                if let Some(handle) = options.runtime {
                    downloader = downloader.with_runtime(handle);
                }

                if let Some(value) = options.target_memory_usage {
                    downloader = downloader.with_target_memory_usage(value);
                }

                if let Some(value) = options.max_parallel_streams {
                    downloader = downloader.with_max_parallel_streams(value);
                }

                let verify_method = if options.fast_verify {
                    SophonDownloaderVerifyMethod::Fast
                } else {
                    SophonDownloaderVerifyMethod::Full
                };

                downloader = downloader.with_verify_before_downloading(verify_method);

                // Download game component.
                downloader.download(
                    &download_manifest,
                    &game.path,
                    Arc::new(move |message| {
                        (options.progress)(match message {
                            SophonDownloaderProgressMsg::Verify {
                                current,
                                total,
                                path,
                                result
                            } => SophonUpdaterProgressMsg::Verify {
                                current,
                                total,
                                path,
                                result
                            },

                            SophonDownloaderProgressMsg::Download {
                                current,
                                total
                            } => SophonUpdaterProgressMsg::Download {
                                current,
                                total
                            }
                        })
                    })
                ).await?;

                Ok(())
            }

            Self::Update { game, from_version, to_version } => {
                let Some(update_manifest) = game.api.game(game.region, None, game.game_id.clone())
                    .package(Some(to_version.clone())).await?
                    .find_update_manifest(component).await?
                else {
                    anyhow::bail!("failed to find package update manifest");
                };

                // Prepare sophon updater.
                let mut updater = SophonUpdater::default();

                if let Some(handle) = options.runtime {
                    updater = updater.with_runtime(handle);
                }

                if let Some(value) = options.target_memory_usage {
                    updater = updater.with_target_memory_usage(value);
                }

                if let Some(value) = options.max_parallel_streams {
                    updater = updater.with_max_parallel_streams(value);
                }

                let verify_method = if options.fast_verify {
                    SophonUpdaterVerifyMethod::Fast
                } else {
                    SophonUpdaterVerifyMethod::Full
                };

                updater = updater
                    .with_verify_before_updating(verify_method)
                    .with_verify_before_patching(verify_method);

                // Update game component.
                let chunks_dir = options.chunks_dir.unwrap_or_else(|| {
                    std::env::temp_dir().join("sophon-chunks")
                });

                updater.update(
                    &update_manifest,
                    from_version,
                    &chunks_dir,
                    &game.path,
                    options.progress
                ).await?;

                Ok(())
            }
        }
    }
}

#[derive(Clone)]
pub struct GameDiffOptions {
    /// Copy of tokio runtime handle which should be used by downloader and
    /// updater.
    pub runtime: Option<TokioHandle>,

    /// Name of the sophon component to download. If `None`, then `game` is
    /// used.
    pub component: Option<String>,

    /// Target memory usage is the amount of system memory downloader will try
    /// to use for downloading files’ chunks. The actual usage may be higher,
    /// but should not be lower if there’s enough chunks to download.
    pub target_memory_usage: Option<u64>,

    /// Max parallel streams is the maximal allowed amount of game assets which
    /// can be downloaded in parallel at any given moment. This limit is applied
    /// on top of `target_memory_usage` and is needed to fight soft OS limits on
    /// maximal amount of files open by a single process.
    pub max_parallel_streams: Option<u16>,

    /// Path to the game chunks directory. If unset, temp directory is used.
    pub chunks_dir: Option<PathBuf>,

    /// Verify files by size only instead of calculating their MD5 hashes.
    pub fast_verify: bool,

    /// Updater messages handler.
    pub progress: Arc<dyn Fn(SophonUpdaterProgressMsg) + Send + Sync>
}
