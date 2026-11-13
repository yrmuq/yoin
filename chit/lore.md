# lore

## Protected Areas
- Do not add Qt or a browser engine. Windows stay in `ui/` through `slint`, and the tray stays `ksni`, because yoin is a native Plasma tray program.

## Required Behavior
- A Home Assistant notification is missed when yoin is not running. That is the local push channel, not a delivery bug.
- The Dev tab stays hidden until the logo in About is clicked five times within two seconds. Another five clicks hide it, and opening it does not select it.

## Compatibility Workarounds
- The sideways switch icon is drawn to a bitmap before it is rotated, because the UI toolkit rotates bitmaps only. Remove this when an SVG can be rotated in place.
