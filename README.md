# 🦀 Anime Game Core

Common library to control the Anime Game installation, written in Rust

## Features

| Description | Feature |
| - | - |
| Manage games installations (parse versions, check for updates) | default |
| Install games and download updates | `install` |
| Manage voice packages, download and update them | `install` |
| Repair game installations | `install` |
| Check for telemetry | `telemetry` |
| Apply linux patch | `linux-patch` |
| Static binding of dependencies | `static` |

## Supported games

| Name | Feature |
| - | - |
| [An Anime Game](https://github.com/an-anime-team/an-anime-game-launcher-gtk) | `gen-shin` (without dash) |
| ~~[Honkers](https://github.com/an-anime-team/honkers-launcher-gtk)~~ | ~~`hon-kai` (without dash)~~ (highly outdated, not removed just to once easily revive it) |

⚠️ This library does not bind 7z archives format support, and would require `7z` binary available in user's system. This format may be used in games like honkers
