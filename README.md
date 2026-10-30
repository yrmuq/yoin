<div align="center">
  <img src="assets/icon.png" alt="yoin" width="96">

  <h1>yoin</h1>

  [![License: GPL v3+](https://img.shields.io/badge/License-GPLv3%2B-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
  [![Latest Release](https://img.shields.io/github/v/release/yrmuq/yoin?include_prereleases)](https://github.com/yrmuq/yoin/releases)

  <p>
    A native KDE tray program for Home Assistant. It keeps a websocket open, shows up to ten buttons, and delivers notifications through the <code>mobile_app</code> local push channel. No browser engine and no HTML5 push.
  </p>
</div>

---

> [!NOTE]
> **Active development:** `yoin` is still in active development. `x86_64` AppImage builds are on the [Releases](https://github.com/yrmuq/yoin/releases) page.
>
> **Independent Project:** `yoin` is an independent project and is not affiliated with or endorsed by Home Assistant or the Open Home Foundation.

---

## Features

* **KDE tray:** A StatusNotifier icon. The first launch opens settings. After Home Assistant accepts the token, a left click opens a small rounded window just above the panel, next to the tray. A button click leaves that window open. Each button shows the entity's `on` or `off` state.
* **Buttons:** One by default, up to ten. The Buttons tab sets the order, and that order is the order in the tray window. A button toggles a light, switch, cover, fan, lock, alarm, climate, or anything else Home Assistant can turn between two states. A script is started.
* **Notifications:** `yoin` registers its own `mobile_app` device and subscribes to `mobile_app/push_notification_channel`. Home Assistant delivers `notify.mobile_app_<device>` over the websocket. The desktop notification center shows it. There is no HTML5 registration and no cloud push: if `yoin` is not running, the notification is missed.
* **Session start:** Settings can write an XDG autostart entry that waits until the Plasma panel is up.
* **Persistent configuration:** The address, token, device id, and button choices are stored in `~/.config/yoin/config.json` (`0600`).

---

## Controls & Shortcuts

| Input | Action |
| :--- | :--- |
| Left click (tray) | Open or close the button window. |
| Right click (tray) | Settings and Quit. |
| Button | Run the bound entity. The window stays open, and the label follows the Home Assistant state. An empty button opens Settings. |

Send a notification from Home Assistant with the device name chosen in Settings. A device named `Fedora` is `notify.mobile_app_fedora`.

```yaml
action: notify.mobile_app_fedora
data:
  title: Door
  message: The door is open.
```

---

## Installation & Usage

### Architecture & Format Support

| Format | `x86_64` | `arm64` |
| :--- | :---: | :---: |
| `AppImage` | ✅ | ❌ |
| `Flatpak` | ❌ | ❌ |

### AppImage

The pre-release `AppImage` is a Rust binary. It does not bundle Qt, Python, or a browser. Download the latest `.AppImage` from the [Releases](https://github.com/yrmuq/yoin/releases) page.

For a menu entry, use `Gear Lever` ([Flathub](https://flathub.org/apps/it.mijorus.gearlever)).

Alternatively, grant execution permissions and launch it from the terminal:

```sh
chmod +x yoin-x86_64.AppImage
./yoin-x86_64.AppImage
```

If the host system lacks `FUSE` support, extract and run the bundle directly:

```sh
./yoin-x86_64.AppImage --appimage-extract-and-run
```

### Flatpak

`Flatpak` packaging is planned for a future stable release.

---

## License & Attribution

### Repository License

`yoin` is free software distributed under `GPL-3.0-or-later`. See `LICENSE` and `NOTICE`. Bundled `AppImage` component licenses are listed in `packaging/appimage/BUNDLED.txt`.

### Third-Party Components

| Component | Upstream | License | Integration Details |
| --- | --- | --- | --- |
| `slint` | [slint-ui/slint](https://github.com/slint-ui/slint) | `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` | The windows. FemtoVG draws them. Linked into the binary. |
| `ksni` | [iovxw/ksni](https://github.com/iovxw/ksni) | `Unlicense` | The StatusNotifier tray icon. Linked into the binary. |

*Copyright (C) 2026 yrmuq.*
