# 🦀 Anime Game Core

Unified library to controll different games installations. Provides basic instruments for adding support for mechanics like game updating 

## Features

| Description | Feature |
| - | - |
| Manage games installations (parse versions, check for updates) | default |
| Install games and download updates | `install` |
| Manage voice packages, download and update them | `install` |
| Repair game installations | `install` |
| Apply linux patches | `linux-patch` |

## Supported games

| Name | Feature |
| - | - |
| [An Anime Game](https://github.com/an-anime-team/an-anime-game-launcher) | `gen-shin` (without dash) |
| [The Honkers Railway](https://github.com/an-anime-team/the-honkers-railway-launcher) | `star-rail` |
| [Honkers](https://github.com/an-anime-team/honkers-launcher) | `hon-kai` (without dash) |
| PGR | `pgr` |

⚠️ This library does not bind 7z archives format support, and would require `7z` binary available in user's system. This format may be used in games like honkers
