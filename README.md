<div align="center">

  <img src="assets/icon.png" alt="yoin" width="96">

  <h1>yoin</h1>

  [![License: GPL v3+](https://img.shields.io/badge/License-GPLv3%2B-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
  [![Latest Release](https://img.shields.io/github/v/release/yrmuq/yoin)](https://github.com/yrmuq/yoin/releases/latest)

</div>

---

> [!NOTE]
> `yoin` is in active development. Behavior can change, bugs may occur, and saved settings may become outdated.

---

A native KDE tray program for Home Assistant. It keeps a websocket open, shows up to ten buttons, and receives notifications through the `mobile_app` local push channel. The windows are `slint`. There is no browser engine.

## Features

- Up to ten tray buttons for Home Assistant entities that toggle between two states, plus scripts.
- Notifications through the device's `notify.mobile_app_<name>` service. If `yoin` is not running, the notification is missed. There is no cloud push.
- Optional start with the Plasma session.
- Settings, the token, and the button list stay in `~/.config/yoin/config.json` (`0600`).

## Controls & Shortcuts

<details>
<summary>Controls</summary>

| Input | Action |
| :--- | :--- |
| Left click (tray) | Open or close the button window. |
| Right click (tray) | Settings and Quit. |
| Button | Run the bound entity. The window stays open. An empty button opens Settings. |

A notification uses the device name from Settings. `Fedora` is `notify.mobile_app_fedora`.

</details>

---

## Installation & Usage

### Architecture & Format Support

| Format | `x86_64` | `arm64` |
| :--- | :---: | :---: |
| `AppImage` | ✅ | ❌ |
| `Flatpak` | ❌ | ❌ |

### Release Scheme

Stable releases are `X.Y.0`. A version `X.Y.Z` with `Z` greater than 0 is an automatic dependency-update prerelease. See the [latest stable release](https://github.com/yrmuq/yoin/releases/latest) and [all releases](https://github.com/yrmuq/yoin/releases).

### AppImage

`yoin` needs a Plasma session with a StatusNotifier tray.

Download the latest stable `.AppImage` from the [latest release](https://github.com/yrmuq/yoin/releases/latest). Other builds are on the [releases page](https://github.com/yrmuq/yoin/releases).

Install and update it with [Gear Lever](https://flathub.org/apps/it.mijorus.gearlever). The `.AppImage` contains the update information, and the `.zsync` file beside it is used for a smaller download. Import only the `.AppImage`. Do not open the `.zsync` file. Gear Lever checks for updates; this does not by itself update the program while you are not using it.

### Flatpak

Flatpak support is planned.

---

## License & Attribution

`yoin` is free software under [`GPL-3.0-or-later`](LICENSE). Notices are in [`NOTICE`](NOTICE). Bundled AppImage component licenses are in `packaging/appimage/BUNDLED.txt`.

| Component | Upstream | License | Integration Details |
| --- | --- | --- | --- |
| `slint` | [slint-ui/slint](https://github.com/slint-ui/slint) | `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` | The windows. FemtoVG draws them. Linked into the binary. |
| `ksni` | [iovxw/ksni](https://github.com/iovxw/ksni) | `Unlicense` | The StatusNotifier tray icon. Linked into the binary. |
| `resvg` | [linebender/resvg](https://github.com/linebender/resvg) | `Apache-2.0 OR MIT` | Draws the one icon that has to be rotated. Linked into the binary. |
