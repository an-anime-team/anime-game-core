# 🦀 Anime Game Core

Common library to control the Genshin Impact installation, written in Rust

## Roadmap to 1.0.0

* [ ] Game
  * [x] Get current version
  * [x] Calculate difference with the latest version

  Feature: `install`

  * [x] Install the difference
  * [ ] Apply changes for updates
    * [x] Remove outdated files
    * [ ] Apply hdiff changes
  * [ ] Repair game files

* [ ] Voice packages
  * [x] List installed packages
  * [x] Get packages versions
  * [x] List available packages
  * [ ] Calculate difference with the latest version

  Feature: `install`

  * [ ] Install the difference
  * [ ] Apply changes for updates
    * [ ] Remove outdated files
    * [ ] Apply hdiff changes
  * [ ] Delete voice packages
  * [ ] Repair broken packages

Feature: `telemetry`

* [ ] Disable / enable

Feature: `linux-patch`

* [ ] Identify installed patch info
* [ ] Fetch remote patch info
* [ ] Apply / revert patch

Feature: `wine`

* [ ] Manage wine installations (download, remove)
* [ ] Create prefix

Feature: `dxvk`

* [ ] Manage DXVKs installations (download, remove)
* [ ] Apply DXVK
