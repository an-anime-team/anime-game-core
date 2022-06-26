# 🦀 Anime Game Core

Common library to control the Anime Game installation, written in Rust

## Roadmap to 1.0.0

* [ ] Game
  * [x] Get current version
  * [x] Calculate difference with the latest version

  Feature: `install`

  * [x] Install the difference
  * [ ] Apply changes for updates
    * [x] Remove outdated files
    * [ ] Apply hdiff changes
  * [x] Repair game files

* [ ] Voice packages
  * [x] List installed packages
  * [x] Get packages versions
  * [x] List available packages
  * [x] Calculate difference with the latest version

  Feature: `install`

  * [x] Install the difference
  * [ ] Apply changes for updates
    * [x] Remove outdated files
    * [ ] Apply hdiff changes
  * [ ] Delete voice packages
  * [ ] Repair broken packages

Feature: `telemetry`

* [ ] Disable / enable

Feature: `linux-patch`

* [x] Fetch remote patch info
* [x] Identify installed patch info
* [x] Apply / revert patch

Feature: `wine`

* [ ] Manage wine installations (download, remove)
* [ ] Create prefix

Feature: `dxvk`

* [ ] Manage DXVKs installations (download, remove)
* [ ] Apply DXVK
