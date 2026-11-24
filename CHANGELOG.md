# Changelog

What changed in each version of PaletteCraft (called Painting Colour Assistant up to
version 0.3.2). The newest version is at the top.

## Unreleased

- **Spectral paint mixing.** Recipes and the mixing chart now mix paints wavelength by
  wavelength (spectral Kubelka-Munk) instead of only by red, green and blue, which comes
  closer to real paint: complementary colours neutralise to grey, a red with a blue leans
  violet, diluted greens stay green. Recipes may change. The *Mixing model* switch beside
  *Paints per mix* goes back to the earlier RGB mixing; each picture remembers its choice,
  and the PDF brochure names the model used.

- **Remove one picture from the recent list.** Each picture in the list beside
  *Open picture...* has a × that removes it, with its saved settings; the list stays open,
  so you can remove several. In the web app this also deletes its copy in the browser.
  *Clear recent list* still removes all of them.

## 0.4.0 – 21 November 2026

- **New name: PaletteCraft.** The app, its downloads and its website have a new name and
  address: [skrilic.github.io/PaletteCraft](https://skrilic.github.io/PaletteCraft/).
  Palettes and the recent list start empty under the new name.

## 0.3.2 – 20 November 2026

- **Recently opened pictures in the web app.** The arrow beside *Open picture...* now lists
  the last 10 pictures there too, each with its settings. The web app keeps a copy of
  them in the browser on your device; *Clear recent list* deletes them.
- **Whole recipes in a wide window.** A recipe with long paint names no longer gets cut
  off in the recipe list or under *Clicked spot*: it takes two lines, as on a phone.

## 0.3.1 – 20 November 2026

- **Open picture works in Safari and on iPhone and iPad.** In the web app the button did
  nothing in Safari and in every browser on iOS. *Export PDF* and *Support on Ko-fi* there
  now ask for one more tap instead of doing nothing.
- **Version and update check.** The top bar shows the app's version under its name. Click
  the name (on a phone, the logo) to see whether a newer version is out, with a link to
  download it; the web app reloads with the new version instead. The app asks GitHub only
  when you click; see the privacy policy.

## 0.3.0 – 20 November 2026

- **Web app for phones, tablets and any browser.** Open it at
  [skrilic.github.io/PaletteCraft/app/](https://skrilic.github.io/PaletteCraft/app/)
  and install it from the browser. It works without a network connection once opened.
- **Layout for phones and tablets.** On a narrow screen the app shows one pane at a time:
  the picture with the recipe of the spot you tapped, the picture settings, or the recipes.
  Lists and panes scroll by dragging.
- **Export PDF.** Saves a reference brochure: the picture, the value study, the contour
  drawing, your palette with its mixing chart and a recipe for every colour level.
- **Contours view.** A line drawing of the picture's edges, with adjustable detail. It can
  also be drawn on top of the other views.
- **Golden ratio grid.** A composition overlay with lines at 38.2% and 61.8%.
- **Recently opened pictures.** The arrow beside *Open picture...* lists the last 10
  pictures, and each picture remembers its crop, views, overlay and paints.
- **Match ratings explained.** Point at or tap a rating such as *good* to see what the
  colour difference means for your painting.
- **Support on Ko-fi.** A button in the top bar, if you would like to leave a tip.
- **Project website** with downloads, this changelog and a privacy policy. The web app
  moved to its own address, ending in `/app/`.

## 0.2.0 – 17 November 2026

- The app has its own icon on every system.
- Licensed as MIT or Apache-2.0, at your choice.

## 0.1.0 – 17 November 2026

The first version.

- **Palettes.** Build palettes for watercolour, acrylic and oil from a library of 45
  common pigments, or enter your own paints. Each palette shows a chart of its paints
  mixed in pairs.
- **Picture views.** Original, greyscale, value levels (2 to 10) and colour levels (2 to 24).
- **Crop** with free or fixed shapes.
- **Composition overlays.** Rule of thirds, Fibonacci spiral and a square grid.
- **Mixing recipes** for every colour level and any spot you click, made from the paints in
  your palette with a paint-mixing model (Kubelka-Munk), with a match rating.
- Downloads for Linux, macOS and Windows.
