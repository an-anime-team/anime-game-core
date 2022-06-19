# 🦀 Anime Game Core

Common library to control the Genshin Impact installation, written in Rust

## Roadmap to 1.0.0

- Game installation
  - <s>Current game version</s>
  - <s>Latest game version</s>
  - <s>Versions differences</s>

  Feature: `install`

  - Install the game
  - Update existing installation
  - Repair game files

- Voice packages
  - <s>Installed voice packages</s>
  - <s>Available voice packages</s>

  Feature: `install`

  - Install new voice packages
  - Delete voice packages
  - Update outdated packages
  - Repair broken packages

Feature: `linux-patch`

- Identify installed patch info
- Fetch remote patch info
- Apply / revert patch

Feature: `wine`

- Manage wine installations (download, remove)
- Create prefix

Feature: `dxvk`

- Manage DXVKs installations (download, remove)
- Apply DXVK
