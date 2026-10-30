# map

Read only the row the task needs.

## Own code
- `src/main.rs` — starts the tray, the button window, notices, and settings.
- `src/tray.rs` — the StatusNotifier icon. Left click toggles the window.
- `src/client.rs` — the Home Assistant websocket: register, call a service, follow state.
- `src/ha.rs` — service URLs and which domains toggle.
- `src/config.rs` — `~/.config/yoin/config.json`.
- `src/notices.rs` — the newest three notices.
- `src/notify.rs` — a desktop notification on the session bus.
- `src/place.rs` — where the window opens, and the KWin rule in `kwin/placer.js`.
- `src/theme.rs` — the KDE color scheme and the font.
- `src/icons.rs` — Breeze icons, including the sideways arrow.
- `src/i18n.rs` — `languages/*.ini`. A line is `id = English original = text shown`.
- `ui/app.slint` — the button window, the notices, and settings.
- `languages/` — `English.ini` and `Russian.ini`.

## Packaging
- `packaging/appimage/build.sh` — builds the Rust image and pins `appimagetool`.
- `.github/workflows/appimage.yml` — builds a version tag. `X.Y.0` is a release. Any other `X.Y.Z` is a prerelease.
- `.github/workflows/release.yml` — a merge commit from `dev` into `main` raises Y and publishes.
- `.github/workflows/dev.yml` — manual draft from `dev`. It does not take a version number.
- `.github/workflows/deps.yml` — Dependabot prerelease on `deps`. It does not change `main`.
- `.github/workflows/pins.yml` — `appimagetool` pin, on the 28th. Same prerelease path, not `main`.
- `packaging/release/` — version bump and notes. Not the program.

## Libraries
- `slint` — the windows. FemtoVG draws them.
- `ksni` — the tray icon.
- `tokio-tungstenite` — the Home Assistant websocket.
- `zbus` — notifications and the KWin rule.
- `resvg` — icon rasterizing.

## Do not open
- `packaging/appimage/build/` — local build output.
- `target/` — compiler output.
