# lore

## Protected Areas
- Do not add Qt or a browser engine. Windows stay in `ui/` through `slint`, and the tray stays `ksni`, because yoin is a native Plasma tray program.

## Required Behavior
- A Home Assistant notification is missed when yoin is not running. That is the local push channel, not a delivery bug.
- The developer panel stays hidden until the version line in About is clicked five times within two seconds.

## Compatibility Workarounds
- The sideways switch icon is drawn to a bitmap before it is rotated, because the UI toolkit rotates bitmaps only. Remove this when an SVG can be rotated in place.
