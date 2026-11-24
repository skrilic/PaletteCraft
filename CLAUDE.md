# CLAUDE.md

PaletteCraft: a Rust + macroquad desktop app that turns a reference picture
into value/colour studies, composition overlays and Kubelka-Munk mixing recipes from the
user's paint palettes. See README.md for the feature overview.

## Layout

- All source lives in `app/` (a single binary crate, edition 2024). Run cargo from `app/`.
- `app/vendor/miniquad/` is a patched miniquad 0.4.11 used via `[patch.crates-io]`.
- Root files `craft.json`, `feed.json`, `investigation.cany/`, `checks/`, `capture/` are
  state from the "Craft Anything" tool that generated the project. Do not edit them by hand.

## Commands (from `app/`)

```sh
cargo build
cargo test                                    # km.rs unit tests
cargo run -- --self-test shots/selftest.json  # scripted end-to-end test
cargo run -- --screenshot shots/now.png       # render sample landscape to PNG
cargo run -- --palettes                       # open on the Palettes screen
cargo run -- <image path>                     # open with a picture
cargo run -- --screenshot x.png --window 390x780 --pane 2   # compact (phone) layout, pane 0-2
./web/build.sh                                # web build (wasm32) into target/web/
python3 -m http.server -d target/web 8080     # serve it locally
```

- The self-test always exits 0. Read the JSON: every step must have `"passed": true`.
- `--screenshot` / `--self-test` open a real window and use `palette::default_store()`;
  they never read or write the user's `palettes.json` or `recent.json` (`App::persist` is false).
- After UI changes, run both the self-test and a screenshot and look at the PNG. Check the
  compact layout too (`--window 390x780`, `--pane 0..2`, with and without `--palettes`).
- The web build has no self-test or screenshot mode; check it in a browser. It needs
  `rustup target add wasm32-unknown-unknown`. Keep both `cargo build` and
  `cargo build --target wasm32-unknown-unknown` warning-free.

## Releases

- `.github/workflows/release.yml` builds on tag `v*` (Linux on ubuntu-22.04, universal
  macOS `.app` via `lipo` on macos-latest, Windows `.zip`) and publishes with `gh release create`.
  Tag must equal `app/Cargo.toml` version. Manual dispatch builds without publishing.
- `.github/workflows/web.yml` builds the website with the web app under `app/` and
  publishes it on GitHub Pages (Pages source: GitHub Actions). It runs on manual dispatch
  and as the last job of `release.yml` (after `gh release create`, so the download buttons
  find the new files); runs from a tag need the github-pages environment to allow `v*`.
- Website (`site/`, served at the Pages root): `build.py` (stdlib only) fills `layout.html`
  with `home.html`, `CHANGELOG.md` and `PRIVACY.md`, takes the download links from the
  latest release via `gh`, and copies the built web app into `app/`. Local build:
  `python3 site/build.py --app app/target/web` (output `_site/`, git-ignored).
  `site/make_images.py` renders the pictures in `site/img/` with the app (`--view`,
  `--window`). `layout.html` counts visits with GoatCounter (palettecraft.goatcounter.com;
  site pages only, never the web app): mention any change to it in `PRIVACY.md`. Update `CHANGELOG.md` with every user-visible change, and
  `PRIVACY.md` if what the app stores or sends changes.
  Web app icons (`app/web/icons/`) come from `make_icons.py` (`--web-only` renders just
  those; it falls back to `rsvg-convert` without Inkscape). `sw.js` lists every file it
  caches: add new web files there too.
- Packaging inputs live in `packaging/` (macOS `Info.plist` with `@VERSION@`, Linux
  `.desktop`, `release-notes.md` used as the release body).
- App icon: edit SVGs in `packaging/icon/` (`icon.svg` ≥64 px, `icon-32.svg`, `icon-16.svg`
  for small sizes), then run `python3 packaging/icon/make_icons.py` (Inkscape + Pillow) and
  commit the generated `app/assets/*` and `packaging/icon/{AppIcon.icns,icon-256.png}`.
  `main.rs` embeds the PNGs as the window icon (a unit test checks they decode);
  `app/build.rs` embeds `assets/icon.ico` into the Windows .exe via `winresource`.
- `Platform::linux_wm_class` ("palettecraft") must match `StartupWMClass`
  in the `.desktop` file.
- Linux builds need `libwayland-dev` + `pkg-config` (rfd links `libwayland-client`).
- `main.rs` sets `windows_subsystem = "windows"` for release builds, so stdout/stderr
  are invisible there; don't rely on console output for Windows diagnostics.

## Architecture

- `main.rs` – arg parsing, window config, main loop, screenshot capture. Sets
  `MINIQUAD_NO_ACTIVATE=1` in background modes.
- `app.rs` – `App` state (`Screen::{Palettes, Painting}`, `View`, crop, picked colour,
  incremental recipe search). Screens are split into `app/painting_view.rs` and
  `app/palettes_view.rs` as `impl App` blocks.
- `ui.rs` – own immediate-mode widgets. Every widget registers its rect under a string id;
  `selftest.rs` clicks widgets by those ids. **Renaming a widget id breaks the self-test.**
  Popup items are `"<id>#<n>"`; `ui.tooltip` is drawn last in `Ui::end` (tabs = columns).
  `ui.clicked` fires on release without dragging (a tap), so drags can scroll on touch
  screens; use `ui.pressed_in` for things that are dragged (sliders, crop frame).
  Scroll with `ui.scroll_rows` (row lists) or `ui.scroll_px` (whole panes), not the wheel alone.
- Compact layout (`App::compact()`, window narrower than `COMPACT_W`): one pane at a time,
  picked in the pane bar (`pane_painting` / `pane_palettes`). Panels lay out by their rect's
  width, so the same panel functions serve both layouts; content scrolled under the bars is
  covered by them and `ui.set_clip_top` keeps it from reacting. The self-test runs at
  1400 x 880 only, so it does not cover the compact layout.
- `selftest.rs` – queue of `Action`s (click, drag, type, wait, check) producing synthetic
  `Input` per frame; checks read `App` state directly.
- `km.rs` – two-constant Kubelka-Munk per channel: `MixModel::Spectral` (default, the
  `spectral::BANDS` wavelength bands) or `MixModel::Rgb` (linear RGB); `Mixer::new(palette,
  model)`. Watercolor = film over paper
  (`PAPER`), lightened by `WATER_LOADS`; acrylic/oil = `r_inf`, lightened with white
  (`WHITE_PARTS`). Recipe search is time-sliced (`RECIPE_BUDGET_MS` per frame).
  The model is `App::mix_model`, kept in `PictureSettings` (`#[serde(default)]`).
- `spectral.rs` – sRGB → smoothest reflectance curve (Burns' LHTSS, Newton, cached per
  colour), curve → linear sRGB (CIE 1931 multi-lobe fit, 6504 K black body, rows scaled
  so a white reflector is white). Unit tests round-trip the whole sRGB cube.
- `imgproc.rs` – working copy capped at `WORK_MAX_SIDE`; crops apply to the full original.
  Colour levels = seeded k-means++ in Lab (deterministic).
- `colour.rs` – sRGB/linear/Lab, CIE76 ΔE. `palette.rs` – `Paint`, `Palette`, `Store`,
  45-pigment `LIBRARY`, JSON persistence through `storage.rs`. `recent.rs` – recently opened
  pictures with their `PictureSettings` (`recent.json` beside `palettes.json`). `App`
  stores the open picture's settings each frame no mouse button is held, and in `set_image`
  before switching; `open_path` restores them. The web build keys its pictures as
  `browser-<hash>/<name>` (`recent::browser_key`) and keeps copies of those in the menu in
  IndexedDB (palettecraft.js `pc_picture_store` / `pc_pictures_keep` / `pc_picture_load`); a
  picture opened from the menu comes back through `web::take_picture` (empty when gone). A new per-picture setting goes in
  `PictureSettings`, `App::picture_settings` and `App::apply_settings`.
- `storage.rs` – named JSON texts: files in `dirs::config_dir()/PaletteCraft/` on
  the desktop, `localStorage` keys `PaletteCraft/<name>` on the web.
- Web build (`target_arch = "wasm32"`): `web.rs` declares JS imports implemented in
  `web/palettecraft.js`, a miniquad plugin loaded after the vendored `gl.js` (open picture via
  `<input type=file>`, PDF as a download, local storage, and a see-through `<input>` laid over
  each visible text field so phones show their keyboard; `Ui::text_field` reports its rect
  every frame and `Ui::end` hides the inputs of fields not drawn). When the browser no
  longer counts an action as the user's (`navigator.userActivation`), palettecraft.js shows a small
  sheet with a real button for one more tap. WebKit (Safari, every iOS browser) opens a
  file chooser only inside the click or tap handler itself, so palettecraft.js always shows the
  sheet there, and `Ui::picture_button` ("Open picture...") lays a see-through `<label>`
  for the file input over the button each frame, so the tap goes straight to the browser.
  `web::frame_end` (in `Ui::end`) hides the inputs and that label when not drawn. `rfd` and `dirs` are desktop-only
  dependencies; code using them is `cfg`-gated. Don't use `std::time::{Instant, SystemTime}`
  (they panic in wasm); use `miniquad::date::now()`. The web build embeds
  `assets/fonts/DejaVuSans.ttf`. Bump `PLUGIN_VERSION` in `web.rs` together with `version`
  in `palettecraft.js` when the import list changes.
- `update.rs` – update check: `VERSION` (from Cargo.toml), `UpdateCheck` asks GitHub's
  latest-release API for `tag_name`, only when the user clicks the app's name (compact:
  the logo, widget id `about`). Desktop: a thread with `ureq` (desktop-only dependency);
  web: `fetch` in palettecraft.js. `app/update_view.rs` draws the "About" dialog; while it is open
  `Ui::set_covered` keeps everything under it (and the web inputs) from reacting. "Download"
  opens the website (`DOWNLOAD_URL`); on the web "Update now" reloads through the service
  worker. `cargo test -- --ignored` runs the one test that needs the network. The check is
  described in `PRIVACY.md`: update it if what is sent changes.
- `links.rs` – `open_url` (desktop: `open` / `xdg-open` / `cmd /C start`; web: palettecraft.js) and
  `KOFI_URL`, used by the "Support on Ko-fi" button (top bar; compact layout: bottom of
  the Palettes pane). `.github/FUNDING.yml` has the same Ko-fi page.
- `grid.rs` – rule of thirds, golden ratio (phi) grid, Fibonacci spiral, square grid overlays.
  `Overlay` is serialised in `recent.json`: add new kinds at the end of the enum.
- `pdf.rs` – minimal PDF writer (JPEG images, shapes, Helvetica text with built-in widths).
  `app/export.rs` lays out the reference brochure; "Export PDF" uses the rfd save dialog,
  or `App::export_path_override` (set to a temp file by the self-test).

## Conventions

- Model and UI constants are documented in `app/MECHANICS.md` and `app/mechanics.json`
  with a source (request or estimated). When you change a constant, update both files.
- Use British "colour" in code and UI text.
- Keep the build warning-free, including the vendored miniquad.

## Vendored miniquad

Local changes vs upstream 0.4.11 (keep this list current when touching `vendor/`):

1. `src/native/macos.rs`: honour `MINIQUAD_NO_ACTIVATE` (don't activate/focus the window).
2. `src/lib.rs`: `set_or_replace_display` is `#[cfg(target_os = "android")]` (dead code elsewhere).
3. `src/native/linux_x11/clipboard.rs`: `MESSAGE` is a `Mutex<Option<String>>` instead of
   `static mut` (fixes the `static_mut_refs` warning).
4. `src/native/wasm.rs`: the `resize` export re-reads `dpi_scale()` (browser zoom or a move
   to another screen changes `devicePixelRatio`; upstream keeps the value from start-up).
