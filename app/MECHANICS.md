# PaletteCraft: mechanics sheet

There is no original program to match: everything below comes from the person's request
("request") or is a design choice made while building ("estimated"). The same values are
in `mechanics.json`.

## Flow and controls

| What | Value | Source |
|---|---|---|
| Screens | 1 Palettes, 2 Painting (tabs in the top bar) | request: "artist creates palettes at the beginning" |
| First start | opens on Palettes; later starts open on Painting | estimated |
| Open a picture | "Open picture..." file dialog, drop a file on the window, or pass a path on the command line | request |
| Recently opened | arrow button beside "Open picture..." lists recent pictures, newest first (file name and last two folders, "not found" if missing), each with a × that removes it and its settings (the menu stays open), then "Clear recent list"; every picture opened from a file is added | request |
| Recent list length | 10 pictures in the menu; settings kept for the last 100 pictures | estimated |
| Settings per picture | crop, view, value levels, colour levels, contour detail, lines on top, overlay (kind, grid cells, diagonals, mirroring, line colour and strength), paint type, palette (by name; first palette of that paint type if it is gone), paints per mix, lightening, mixing model, clicked spot; restored whenever that picture is opened again from a file. New pictures start with the current settings and no crop | request |
| Settings saved | after every finished change (no mouse button held) and when another picture is opened; "Clear recent list" forgets the settings too | estimated |
| Recent list saved | JSON next to the palettes (`PaletteCraft/recent.json`); `--screenshot` / `--self-test` never read or write it | estimated |
| Buttons, list rows, menus | react on release (tap); a press that moves more than 10 px before release is a drag, not a tap (so dragging a list on a touch screen does not press what is under the finger). Sliders and the crop frame react on press | estimated |
| Pick a colour | click or tap on the picture (Original / Greyscale / Value views) gives a recipe for a 5x5 px average | estimated |
| Select a colour level | left click on the picture or a recipe row (Colour levels view) fades the other levels | estimated |
| Crop | "Crop..." then drag: outside the frame = new frame, inside = move, corner handle (12 px) = resize; "Apply crop", "Undo crop" | request |
| Crop shapes | Free, 1:1, 4:5, 3:4, 2:3, 5:4, 4:3, 3:2, 16:9 | estimated |
| Scroll lists | mouse wheel, 1 row per notch (library: 2 rows), or drag the list (one row per row height dragged) | estimated |
| Text fields | click to type, Backspace deletes, Enter/Escape/click elsewhere finishes; in the web build a browser input lies over the field, so phones and tablets show their keyboard | estimated |
| Delete palette | needs a second click ("Click again") | estimated |
| Palettes saved | automatically after every change, JSON in the user settings folder (`PaletteCraft/palettes.json`) | estimated |

## Picture analysis

| What | Value | Source |
|---|---|---|
| Working copy longest side | 1400 px (bigger pictures are scaled down for display and analysis; crops use the full original) | estimated |
| Views | Original, Greyscale, Value levels, Colour levels, Contours | request |
| Value levels | default 5, range 2 to 10 | request (number chosen by artist); default estimated |
| Value banding | L* (CIE lightness) between the 0.5% and 99.5% percentiles split in equal steps; shown as evenly spaced greys black to white | estimated |
| Value scale percentages | share of the picture's area (pixels of the working copy) in each value level; they add up to 100%; caption beside the boxes, pop-up on hover | estimated |
| Greyscale | each pixel shown as neutral grey of the same L* | estimated |
| Colour levels | default 8, range 2 to 24 | request; default estimated |
| Colour reduction | k-means++ in CIE Lab, 60 000 sampled pixels, 12 iterations, fixed seed; levels sorted dark to light; shown colour = mean sRGB of its pixels | estimated |
| Contours | Canny-style edges on smoothed Lab (a*, b* weighted 0.5 against L*): Sobel gradient, thinning to ridges, hysteresis (weak edges down to 0.45 of strong), short pieces dropped | request (contours); method estimated |
| Contour detail | default 5, range 1 to 10 | estimated |
| Contour smoothing radius | ceil((11 - detail) / 2) + 1 px of the working copy, two box-blur passes | estimated |
| Contour seed share | 0.05 + 0.04 x detail of the edge ridges | estimated |
| Contour minimum length | max(6, 40 - 3 x detail) px | estimated |
| Contour line thickening | longest side / 900 px on each side (1 px for the 1400 px working copy) | estimated |
| Contours view | graphite lines (46, 46, 52) on paper (247, 244, 236) | estimated |
| Lines on top | contour lines over any other view: dark on light areas, light on dark areas | estimated |

## Kubelka-Munk mixing

Two-constant Kubelka-Munk. Each paint is described by its masstone colour (full
strength), its transparency and its tinting strength. With the **spectral** model (the
default) every colour becomes a reflectance curve and K and S are worked out and mixed per
wavelength band; with the **RGB** model the same equations run on the three linear RGB
channels instead.

| What | Value | Source |
|---|---|---|
| Media | Watercolor, Acrylic, Oil | request |
| Mixing model | Spectral (default) or RGB, "Mixing model" switch under "Paints per mix"; kept per picture, pictures saved before it existed get Spectral; hovering explains the two | request: "Spectral Kubelka-Munk for more accurate mixes" |
| Spectral bands | 31, from 400 to 700 nm in 10 nm steps | estimated |
| Reflectance curve of a colour | the smoothest curve between 0 and 1 with exactly that colour (least hyperbolic tangent slope squared, Newton's method); greys are flat. Every sRGB colour has one (unit test over the cube, round trip under 1 dE) | Burns 2020 (method); fit checked by unit test |
| Observer | CIE 1931 2°, multi-lobe Gaussian fit of the colour matching functions | Wyman, Sloan & Shirley 2013 |
| Illuminant | black body at 6504 K (close to D65); weights scaled so a perfect white reflector is sRGB white | estimated |
| Paper, white paint | also turned into curves (paper flat-ish, near-white) | estimated |
| Scattering S, transparent | 0.12 | estimated |
| Scattering S, half-transparent | 0.45 | estimated |
| Scattering S, opaque | 1.6 | estimated |
| Watercolor model | paint layer of thickness = pigment load over paper; K solved (per band or channel) so a full-strength layer shows the masstone | estimated |
| Paper colour | sRGB 247, 244, 236 | estimated |
| Water steps (pigment load) | 1.0, 0.75, 0.55, 0.40, 0.30, 0.22, 0.16, 0.11, 0.075, 0.05, 0.033, 0.02 | estimated |
| Water words | >=0.9 full strength, >=0.65 creamy, >=0.45 like milk, >=0.28 like coffee, >=0.15 like tea, >=0.07 light wash, else very pale wash | estimated (common painters' terms) |
| Acrylic/oil model | opaque body, R = R_inf(K/S); K = S x (1-R)^2 / 2R from the masstone | estimated |
| White for acrylic/oil | the palette's PW paint, otherwise Titanium White sRGB 246, 246, 242 opaque | estimated |
| White parts tried | 0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 40 | estimated |
| Mixture | K and S added by parts x tinting strength | estimated |
| Tinting strength | 0.2 to 3.0, default 1.0 (library values per pigment) | estimated |
| Ratios tried | pairs 1..6 parts each, triples 1..4 parts each (reduced ratios only) | estimated |
| Paints per mix | default 3, choice 1 / 2 / 3 | estimated |
| Simplicity bonus | +0.8 dE per extra paint when ranking | estimated |
| Colour difference | CIE76 dE in Lab | estimated |
| Match words | <2.5 excellent, <5 good, <10 close, else rough | estimated |
| Match explanation | hovering the match label (e.g. "good  (dE 4.2)") shows a pop-up: what dE means, the four bands with this recipe's band highlighted, and that the mix is a model estimate | request |
| Recipe search budget | 12 ms per frame (rest continues next frame) | estimated |

## Palettes

| What | Value | Source |
|---|---|---|
| Paint fields | name, pigment code (e.g. PR206), transparency, masstone colour, tinting strength | request |
| Transparency choices | Transparent, Half-transparent, Opaque | request |
| Starter palettes | Watercolor 1, Watercolor 2, Acrylic 1, Acrylic 2, Oil 1 | request (examples) |
| Watercolor 1 | Crimson PR206 (T), Cobalt Blue PB28 (HT), Yellow Deep PY35 (O), Lemon Yellow PY175, Ultramarine PB29, Burnt Sienna PBr7, Phthalo Green PG7 | request for the first three; codes and the rest estimated |
| Pigment library | 45 common pigments with approximate masstone colours | estimated (not measured) |
| Mixing chart | every pair at 1:1 plus 5 lighter steps per paint (water 1, 0.55, 0.3, 0.16, 0.075 or white 0, 1, 3, 8, 24 parts) | estimated |

## Overlays

| What | Value | Source |
|---|---|---|
| Kinds | None, Rule of thirds, Golden ratio, Fibonacci spiral, Grid | request |
| Rule of thirds | lines at 1/3 and 2/3, rings (7 px) at the four crossings | estimated |
| Golden ratio grid | lines at 1/phi^2 = 0.382 and 1/phi = 0.618 of width and height, rings (7 px) at the four crossings | request (golden ratio grid); drawing estimated |
| Fibonacci spiral | rectangles cut at 1/phi (phi = 1.618034) of the side, 12 steps, quarter-ellipse arcs, stretched to the picture; portrait pictures start with a horizontal cut; mirror sideways / up-down | estimated |
| Grid | square cells, default 6 on the long side, range 2 to 20, optional diagonals, column letters and row numbers | estimated |
| Line colour | White, Black, Red; strength 0.15 to 1.0, default 0.8 | estimated |

## Window

| What | Value | Source |
|---|---|---|
| Window | 1400 x 880, resizable | estimated |
| Left panel / right panel | 250 px / 430 px | estimated |
| Support on Ko-fi | button left of "Export PDF..." (compact layout: bottom of the Palettes pane) opens https://ko-fi.com/Q6X32AWI41 in the browser | request |
| Compact layout | below 900 px window width (phones, tablets held upright): one pane at a time, chosen in a 44 px pane bar under the top bar (Painting: Picture, Adjust, Recipes; Palettes: Palettes, Paints, Edit & library); panes taller than the window scroll by dragging or the wheel | request: "tablets and smartphones as important as desktop"; values estimated |
| Compact Picture pane | picture buttons, the picture, the value or colour strip, then a 104 px card with the clicked spot's recipe (or the selected colour level's, in the Colour levels view); no card when the pane is less than 480 px high (phone held sideways) | estimated |
| Recipe row height | 66 px; 84 px, where the paints may take two lines, on a panel narrower than 380 px or when a recipe does not fit on one line | estimated |

## Export PDF (reference brochure)

| What | Value | Source |
|---|---|---|
| Button | "Export PDF..." at the right of the top bar; asks for the file name and folder (system save dialog), adds ".pdf" if missing | request |
| Default file name | "<picture name> - reference.pdf" | estimated |
| Page size | A4 portrait (595 x 842 pt), 42 pt margins, footer with picture name, date and page number | estimated |
| Page 1 | original picture (cropped if a crop is applied) and the settings, including the mixing model | request |
| Page 2 | value levels picture with the value scale and percentages, contour drawing at the same size | request |
| Page 3 | chosen palette: paints, pigment codes, transparency, tinting strength, 1:1 mixing chart and lightening steps | request |
| Page 4+ | colour levels picture and a recipe for every level, with the chosen paints per mix (1, 2 or 3), plus the clicked spot | request |
| Match rating legend | after the recipe list: what dE means and the four match words with their dE ranges (same text as the app's pop-up) | request |
| Pictures | JPEG quality 90 at the working-copy size (up to 1400 px) | estimated |
| Text | Helvetica (built into PDF readers); letters outside its set lose their accent | estimated |

