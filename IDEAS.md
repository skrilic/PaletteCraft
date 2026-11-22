# Ideas

Possible future features, not yet triaged or planned. Each entry says what the app does
today and what is still open for triage.

## 1. Remove a single picture from the recent list

Today the recent list (the arrow beside *Open picture...*) can only be cleared as a whole
(*Clear recent list*). Proposed: remove one entry, together with its saved settings.

Open for triage:
- How to remove it: an × on each row, or swipe/long press on touch screens.
- In the web app, also delete the picture's copy from IndexedDB (`pc_pictures_keep`
  already deletes copies that are no longer listed).

## 2. Export and import palettes and default settings

Today palettes live in `palettes.json` (desktop) or browser storage (web app), with no way
to move them between devices, or between the desktop app and the web app. A newly opened
picture starts with fixed defaults (5 value levels, 8 colour levels, the app's default
medium); a picture from the recent list gets its own saved settings back.

Proposed:
- Export and import the user's palettes as a JSON file.
- User-chosen defaults for a picture without saved settings: medium (watercolour,
  acrylic, oil), value levels, colour levels; possibly also the palette, overlay, etc.

Open for triage:
- One file for palettes and defaults, or separate files.
- On import: replace, or merge (what happens to palettes with the same name).
- Where the defaults are set: a "Use as default" button on the current settings, or a
  separate settings section.

## 3. Canvas (support) size and crop to its proportions

Today *Crop...* offers fixed aspect ratios (Free, 1:1, 4:5, 3:4, 2:3, 5:4, 4:3, 3:2,
16:9). Proposed: enter the size of the canvas or paper (width × height, e.g. in cm), and
crop offers that ratio. The ratio can be unlocked to crop freely, independent of it.

Open for triage:
- Units (cm / inches) and whether the size is saved per picture (in the recent list).
- Whether the size is also used elsewhere: in the PDF brochure (scale, grid in cm), or for
  a grid over the picture in real units.
- Portrait/landscape: swap width and height with one tap.

## 4. Historical and well-known palettes

Proposed: ready-made palettes to choose from, besides the user's own, for example:
- Zorn palette (yellow ochre, vermilion / cadmium red, ivory black, white),
- Apelles' four-colour palette (white, yellow ochre, red earth, black),
- Caravaggio palette,
- Velázquez palette,
- the simplest palette of three primaries (plus white for acrylic and oil).

Open for triage:
- Which pigments each historical palette contains; sources differ, so each palette
  should state its source. Historical pigments (lead white, vermilion, smalt, ...) need
  modern equivalents from the pigment library, or new library entries.
- Whether these palettes are read-only (to be duplicated and changed), or copied into the
  user's palettes when chosen.
- For which media each one is offered.
