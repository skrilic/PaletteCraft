//! PaletteCraft: value and colour studies of a picture, composition overlays,
//! and Kubelka-Munk mixing recipes from the artist's own paint palettes.

// Release builds on Windows open no console window next to the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod colour;
mod grid;
mod imgproc;
mod km;
mod links;
mod palette;
mod pdf;
mod recent;
mod selftest;
mod spectral;
mod storage;
mod ui;
mod update;
#[cfg(target_arch = "wasm32")]
mod web;

use app::{App, View};
use grid::Overlay;
use macroquad::miniquad::conf::{Icon, Platform};
use macroquad::prelude::*;
use std::path::PathBuf;
use ui::{Input, Ui};

#[derive(Default)]
struct Args {
    screenshot: Option<String>,
    self_test: Option<String>,
    image: Option<PathBuf>,
    palettes: bool,
    /// Window size in logical px, e.g. "390x780" to see the compact (phone) layout.
    window: Option<(i32, i32)>,
    /// Compact layout: pane to show (0 to 2), for screenshots.
    pane: Option<usize>,
    /// Screenshot of this view alone, without overlay or clicked spot (pictures for the website).
    view: Option<View>,
    /// Start with glazing recipes instead of mixtures.
    glaze: bool,
    /// Kind of paint to start with (watercolor, acrylic, oil), for screenshots.
    medium: Option<palette::Medium>,
    /// Screenshots: a few colours in the artist's own library, the first open in the editor.
    my_colours: bool,
}

impl Args {
    fn parse() -> Self {
        let mut a = Args::default();
        let mut it = std::env::args().skip(1);
        while let Some(s) = it.next() {
            match s.as_str() {
                "--screenshot" => a.screenshot = it.next(),
                "--self-test" => a.self_test = it.next(),
                "--palettes" => a.palettes = true,
                "--glaze" => a.glaze = true,
                "--my-colours" => a.my_colours = true,
                "--medium" => {
                    a.medium = it.next().and_then(|v| palette::Medium::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(&v)))
                }
                "--pane" => a.pane = it.next().and_then(|v| v.parse().ok()),
                "--view" => a.view = it.next().and_then(|v| View::ALL.into_iter().find(|k| k.id() == format!("view_{v}"))),
                "--window" => {
                    a.window = it.next().and_then(|v| {
                        let (w, h) = v.split_once('x')?;
                        Some((w.parse().ok()?, h.parse().ok()?))
                    })
                }
                _ if !s.starts_with("--") => a.image = Some(PathBuf::from(s)),
                _ => {}
            }
        }
        a
    }

    fn background(&self) -> bool {
        self.screenshot.is_some() || self.self_test.is_some()
    }
}

fn window_conf(size: Option<(i32, i32)>) -> Conf {
    let (window_width, window_height) = size.unwrap_or((1400, 880));
    Conf {
        window_title: "PaletteCraft".to_owned(),
        window_width,
        window_height,
        high_dpi: true,
        window_resizable: true,
        icon: window_icon(),
        platform: Platform {
            // Matches StartupWMClass in the Linux .desktop file, so desktops show the app icon.
            linux_wm_class: "palettecraft",
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Title bar / taskbar icon, rendered from packaging/icon by make_icons.py.
fn window_icon() -> Option<Icon> {
    fn rgba<const N: usize>(png: &[u8]) -> Option<[u8; N]> {
        image::load_from_memory(png).ok()?.to_rgba8().into_raw().try_into().ok()
    }
    Some(Icon {
        small: rgba(include_bytes!("../assets/icon-16.png"))?,
        medium: rgba(include_bytes!("../assets/icon-32.png"))?,
        big: rgba(include_bytes!("../assets/icon-64.png"))?,
    })
}

/// The web build has no system fonts to read, so it carries its own (see assets/fonts).
#[cfg(target_arch = "wasm32")]
fn load_font() -> Option<Font> {
    load_ttf_font_from_bytes(include_bytes!("../assets/fonts/DejaVuSans.ttf")).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_font() -> Option<Font> {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Geneva.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    candidates.iter().find_map(|p| std::fs::read(p).ok().and_then(|b| load_ttf_font_from_bytes(&b).ok()))
}

fn save_screenshot(path: &str) {
    let shot = get_screen_data();
    let (w, h) = (shot.width as u32, shot.height as u32);
    let mut img = image::RgbaImage::from_raw(w, h, shot.bytes).expect("screen buffer");
    // OpenGL hands the framebuffer over bottom row first.
    image::imageops::flip_vertical_in_place(&mut img);
    for p in img.pixels_mut() {
        p.0[3] = 255;
    }
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = img.save(path) {
        eprintln!("could not save screenshot: {e}");
    }
}

async fn run(args: Args) {
    let mut ui = Ui::new(load_font());
    let (store, first_start) = if args.background() { (palette::default_store(), false) } else { palette::load_store() };
    let mut app = App::new(store, !args.background(), first_start);
    if let Some(p) = &args.image {
        app.open_path(p);
        app.screen = app::Screen::Painting;
    } else if args.background() {
        app.set_image("Sample landscape".into(), imgproc::sample_landscape(1200, 800));
    }
    if args.glaze {
        app.method = km::Method::Glaze;
    }
    if let Some(m) = args.medium {
        app.select_medium(m);
    }
    if args.screenshot.is_some() {
        app.screen = app::Screen::Painting;
        app.view = View::Colours;
        app.overlay.kind = Overlay::Fibonacci;
        match args.view {
            // set before anything draws the picture (picking a spot builds its texture)
            Some(v) => {
                app.view = v;
                app.overlay.kind = Overlay::None;
            }
            None => app.pick_at(vec2(0.3, 0.86)),
        }
    }
    if args.palettes {
        app.screen = app::Screen::Palettes;
        app.edit_palette = Some(0);
        app.select_paint(Some(0));
    }
    if args.my_colours && args.background() {
        app.store.library = palette::sample_library();
        app.select_lib_colour(Some(0));
    }
    if let Some(p) = args.pane {
        app.pane_painting = p;
        app.pane_palettes = p;
    }
    let mut test = args.self_test.as_ref().map(|_| selftest::SelfTest::new());
    if test.is_some() {
        // The self-test presses "Export PDF"; save to a temporary file instead of asking.
        app.export_path_override = Some(std::env::temp_dir().join("palettecraft-selftest.pdf"));
        // A picture in the recent list (kept in memory only), for the "open recent" step.
        let recent_pic = std::env::temp_dir().join(selftest::RECENT_PICTURE);
        if imgproc::sample_landscape(600, 400).save(&recent_pic).is_ok() {
            app.recent.add(&recent_pic);
        }
    }
    let mut frames = 0u32;
    loop {
        let input = if let Some(t) = &mut test {
            match t.input(&app, &ui) {
                Some(i) => i,
                None => {
                    t.write(args.self_test.as_deref().unwrap());
                    break;
                }
            }
        } else if args.screenshot.is_some() {
            Input { mouse: vec2(-100.0, -100.0), ..Default::default() }
        } else {
            for f in get_dropped_files() {
                // A browser hands over the contents with just a file name as the path.
                match (&f.path, &f.bytes) {
                    (Some(p), _) if p.is_file() => app.open_path(p),
                    (p, Some(b)) => {
                        let name = p.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string());
                        app.open_bytes(name.as_deref().unwrap_or("Dropped picture"), b);
                    }
                    _ => continue,
                }
                app.screen = app::Screen::Painting;
            }
            #[cfg(target_arch = "wasm32")]
            if let Some((name, bytes)) = web::take_picture() {
                app.open_bytes(&name, &bytes);
                app.screen = app::Screen::Painting;
            }
            Input::from_system()
        };
        ui.begin(input, get_frame_time());
        app.frame(&mut ui);
        frames += 1;
        if let Some(path) = &args.screenshot {
            if (frames >= 120 && app.recipes_done()) || frames >= 900 {
                save_screenshot(path);
                break;
            }
        }
        if test.as_ref().is_some_and(|t| t.frames > 20_000) {
            break;
        }
        next_frame().await;
    }
}

fn main() {
    let args = Args::parse();
    if args.background() {
        // Read by the patched window library: open the window without taking focus.
        unsafe { std::env::set_var("MINIQUAD_NO_ACTIVATE", "1") };
    }
    macroquad::Window::from_config(window_conf(args.window), run(args));
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_decodes() {
        assert!(super::window_icon().is_some());
    }
}
