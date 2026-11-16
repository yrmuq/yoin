# lore

## Protected Areas
- Do not add Qt or a browser engine. Windows stay in `ui/` through `slint`, and the tray stays `ksni`, because yoin is a native Plasma tray program.

## Required Behavior
- Only Settings appears in the taskbar; the button and notice windows must stay hidden there, including while native windows are recreated on Wayland.
- A Home Assistant notification is missed when yoin is not running. That is the local push channel, not a delivery bug.
- The Dev tab stays hidden until the logo in About is clicked five times within two seconds. Another five clicks hide it, and opening it does not select it.

## Compatibility Workarounds
- Set each window class before constructing its Slint component: the winit backend can create inactive native windows before show() is polled.
- Create tray panels without decorations and resize borders from the start; changing decorations after the first configure can leave stale Wayland frame extents on the first opening.
- Slint recreates Wayland windows with a temporary default title; KWin must recognize the button and notice panels by their distinct classes before captions arrive.
- The sideways switch icon is drawn to a bitmap before it is rotated, because the UI toolkit rotates bitmaps only. Remove this when an SVG can be rotated in place.

## External Compatibility Constraints
- Preserve KWin’s `Order` when present and remove legacy `rules`/`count` there: Plasma 6.8 migrates a non-empty legacy list by deleting every unlisted rule group, even those listed in `Order`.
- Preserve foreign KWin rule sections and their order; keep a non-overwritten backup before changing `kwinrulesrc`, and abort on read or backup errors.
