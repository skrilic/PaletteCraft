//! The two screens: Palettes (build paint palettes) and Painting (study the picture, get recipes).

mod export;
mod painting_view;
mod palettes_view;
mod update_view;

use crate::colour::{srgb8_to_lab, to_hex};
use crate::grid::{Overlay, OverlaySettings};
use crate::imgproc::{self, Analysis, ColourLevels, ValueLevels};
use crate::km::{MATCH_CLOSE, MATCH_EXCELLENT, MATCH_GOOD, Method, MixModel, Mixer, Recipe, match_label};
use crate::palette::{self, Medium, Palette, Store, Transparency};
use crate::recent::{self, PictureSettings, Recent};
use crate::ui::{self, BAD, BG, BORDER, GOOD, MUTED, PANEL, Pick, TEXT, Ui, WARN, col};
use image::RgbImage;
use macroquad::prelude::*;

pub const TOP_BAR: f32 = 52.0;
pub const LEFT_W: f32 = 250.0;
pub const RIGHT_W: f32 = 430.0;
pub const BOTTOM_STRIP: f32 = 58.0;
pub const RECIPE_ROW_H: f32 = 66.0;
/// Recipe row on a narrow panel, where the paints may take two lines.
pub const RECIPE_ROW_H_NARROW: f32 = 84.0;
/// Time per frame spent on recipe search, so the window stays responsive.
pub const RECIPE_BUDGET_MS: f64 = 12.0;
/// Below this window width (logical px) the compact layout is used: one pane at a time,
/// chosen in a bar under the top bar (phones, tablets held upright, narrow windows).
pub const COMPACT_W: f32 = 900.0;
/// Height of the compact layout's pane bar.
pub const PANE_BAR: f32 = 44.0;
/// Compact layout: height of the clicked-spot card under the picture.
pub const PICKED_CARD_H: f32 = 104.0;

pub const CROP_ASPECTS: [(&str, f32); 9] = [
    ("Free", 0.0),
    ("1:1", 1.0),
    ("4:5", 0.8),
    ("3:4", 0.75),
    ("2:3", 0.6667),
    ("5:4", 1.25),
    ("4:3", 1.3333),
    ("3:2", 1.5),
    ("16:9", 1.7778),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Palettes,
    Painting,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum View {
    Original,
    Greyscale,
    Values,
    Colours,
    Contours,
}

impl View {
    pub const ALL: [View; 5] = [View::Original, View::Greyscale, View::Values, View::Colours, View::Contours];
    pub fn label(self) -> &'static str {
        match self {
            View::Original => "Original",
            View::Greyscale => "Greyscale",
            View::Values => "Value levels",
            View::Colours => "Colour levels",
            View::Contours => "Contours",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            View::Original => "view_original",
            View::Greyscale => "view_grey",
            View::Values => "view_values",
            View::Colours => "view_colours",
            View::Contours => "view_contours",
        }
    }
}

pub struct LoadedImage {
    pub name: String,
    pub original: RgbImage,
    pub crop: Option<(u32, u32, u32, u32)>,
    pub work: RgbImage,
    pub analysis: Analysis,
}

#[derive(Clone, Copy)]
enum CropDrag {
    New(Vec2),
    Move(Vec2),
    Corner(usize),
}

pub struct Picked {
    pub pos: Vec2,
    pub srgb: [u8; 3],
    pub recipe: Option<Recipe>,
}

pub struct App {
    pub store: Store,
    pub persist: bool,
    dirty: bool,
    pub screen: Screen,
    pub medium: Medium,
    pub palette_idx: Option<usize>,

    pub edit_palette: Option<usize>,
    pub edit_paint: Option<usize>,
    hex_edit: String,
    lib_search: String,
    lib_scroll: usize,
    paint_scroll: usize,
    confirm_delete: bool,

    pub img: Option<LoadedImage>,
    pub view: View,
    pub value_levels: i32,
    pub colour_levels: i32,
    pub values: Option<ValueLevels>,
    pub colours: Option<ColourLevels>,
    /// Contour (edge) map of the working copy, computed when shown.
    pub contours: Option<Vec<bool>>,
    pub contour_detail: i32,
    /// Draw the contour lines over the other views too.
    pub contours_on_top: bool,
    texture: Option<Texture2D>,
    tex_dirty: bool,
    pub display_rgba: Vec<u8>,
    crop_tex: Option<Texture2D>,
    pub highlight: Option<usize>,
    pub overlay: OverlaySettings,
    pub overlay_segments: usize,

    pub cropping: bool,
    crop_aspect: usize,
    pub crop_sel: Option<Rect>,
    crop_drag: Option<CropDrag>,

    pub mixer: Option<Mixer>,
    mix_dirty: bool,
    pub recipes: Vec<Option<Recipe>>,
    pub max_paints: i32,
    pub allow_lighten: bool,
    /// Spectral or RGB Kubelka-Munk.
    pub mix_model: MixModel,
    /// Recipes as mixtures or as glazes (transparent layers).
    pub method: Method,
    pub picked: Option<Picked>,
    recipe_scroll: usize,

    status: String,
    status_until: f64,
    /// Where "Export PDF" saves without asking (used by the self-test).
    pub export_path_override: Option<std::path::PathBuf>,
    last_export_dir: Option<std::path::PathBuf>,
    pub canvas_rect: Rect,
    /// Pictures opened lately, newest first, with their settings (saved only when `persist`).
    pub recent: Recent,
    /// Key in `recent` of the open picture; `None` for the sample or a dropped picture without a file.
    pub picture_path: Option<std::path::PathBuf>,
    /// Compact layout: pane shown on the Painting screen (Picture, Adjust, Recipes)...
    pub pane_painting: usize,
    /// ...and on the Palettes screen (Palettes, Paints, Edit & library).
    pub pane_palettes: usize,
    /// Compact layout: how far the current pane is scrolled (px).
    pane_scroll: f32,
    /// Asks GitHub whether a newer version is out (when the "About" dialog opens).
    pub update: crate::update::UpdateCheck,
    /// The "About" dialog is shown (opened from the app's name in the top bar).
    pub about_open: bool,
}

impl App {
    pub fn new(store: Store, persist: bool, first_start: bool) -> Self {
        let mut app = App {
            store,
            persist,
            dirty: false,
            screen: if first_start { Screen::Palettes } else { Screen::Painting },
            medium: Medium::Watercolor,
            palette_idx: None,
            edit_palette: None,
            edit_paint: None,
            hex_edit: String::new(),
            lib_search: String::new(),
            lib_scroll: 0,
            paint_scroll: 0,
            confirm_delete: false,
            img: None,
            view: View::Original,
            value_levels: 5,
            colour_levels: 8,
            values: None,
            colours: None,
            contours: None,
            contour_detail: 5,
            contours_on_top: false,
            texture: None,
            tex_dirty: true,
            display_rgba: Vec::new(),
            crop_tex: None,
            highlight: None,
            overlay: OverlaySettings {
                kind: Overlay::None,
                cells: 6,
                diagonals: false,
                flip_x: false,
                flip_y: false,
                colour: 0,
                opacity: 0.8,
            },
            overlay_segments: 0,
            cropping: false,
            crop_aspect: 0,
            crop_sel: None,
            crop_drag: None,
            mixer: None,
            mix_dirty: true,
            recipes: Vec::new(),
            max_paints: 3,
            allow_lighten: true,
            mix_model: MixModel::Spectral,
            method: Method::Mix,
            picked: None,
            recipe_scroll: 0,
            status: String::new(),
            status_until: 0.0,
            canvas_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            export_path_override: None,
            last_export_dir: None,
            recent: if persist { recent::load() } else { Recent::default() },
            picture_path: None,
            pane_painting: 0,
            pane_palettes: 0,
            pane_scroll: 0.0,
            update: crate::update::UpdateCheck::new(),
            about_open: false,
        };
        app.select_medium(Medium::Watercolor);
        app.edit_palette = if app.store.palettes.is_empty() { None } else { Some(0) };
        app
    }

    fn say(&mut self, s: impl Into<String>) {
        self.status = s.into();
        self.status_until = get_time() + 5.0;
    }

    // ---------- palettes ----------

    pub fn select_medium(&mut self, m: Medium) {
        self.medium = m;
        let keep = self.palette_idx.filter(|&i| self.store.palettes.get(i).is_some_and(|p| p.medium == m));
        self.palette_idx = keep.or_else(|| self.store.palettes.iter().position(|p| p.medium == m));
        self.mix_dirty = true;
    }

    fn palettes_changed(&mut self) {
        self.dirty = true;
        self.mix_dirty = true;
    }

    pub fn current_palette(&self) -> Option<&Palette> {
        self.palette_idx.and_then(|i| self.store.palettes.get(i))
    }

    fn new_palette(&mut self, medium: Medium) {
        let n = self.store.palettes.iter().filter(|p| p.medium == medium).count() + 1;
        self.store.palettes.push(Palette { name: format!("{} {}", medium.label(), n), medium, paints: Vec::new() });
        let i = self.store.palettes.len() - 1;
        self.edit_palette = Some(i);
        self.edit_paint = None;
        self.palettes_changed();
        self.say(format!("New palette \"{}\": add paints from the library", self.store.palettes[i].name));
    }

    pub fn select_paint(&mut self, i: Option<usize>) {
        self.edit_paint = i;
        if let (Some(p), Some(i)) = (self.edit_palette, i) {
            if let Some(paint) = self.store.palettes.get(p).and_then(|pal| pal.paints.get(i)) {
                self.hex_edit = to_hex(paint.color);
            }
        }
    }

    // ---------- image ----------

    pub fn set_image(&mut self, name: String, img: RgbImage) {
        // keep the settings of the picture being closed
        self.remember_settings();
        self.picture_path = None;
        let work = imgproc::fit_working(&img);
        let analysis = imgproc::analyse(&work);
        self.img = Some(LoadedImage { name, original: img, crop: None, work, analysis });
        self.cropping = false;
        self.crop_tex = None;
        self.image_changed();
    }

    fn image_changed(&mut self) {
        self.values = None;
        self.colours = None;
        self.contours = None;
        self.tex_dirty = true;
        self.highlight = None;
        self.picked = None;
        self.recipes.clear();
        self.recipe_scroll = 0;
    }

    fn apply_crop(&mut self) {
        let Some(sel) = self.crop_sel else { return };
        let Some(img) = &mut self.img else { return };
        let (w, h) = img.original.dimensions();
        let x = (sel.x * w as f32).round().clamp(0.0, w as f32 - 1.0) as u32;
        let y = (sel.y * h as f32).round().clamp(0.0, h as f32 - 1.0) as u32;
        let cw = ((sel.w * w as f32).round() as u32).clamp(1, w - x);
        let ch = ((sel.h * h as f32).round() as u32).clamp(1, h - y);
        if cw < 8 || ch < 8 {
            self.say("The crop frame is too small");
            return;
        }
        img.crop = Some((x, y, cw, ch));
        img.work = imgproc::fit_working(&imgproc::crop(&img.original, (x, y, cw, ch)));
        img.analysis = imgproc::analyse(&img.work);
        self.cropping = false;
        self.image_changed();
        self.say(format!("Cropped to {cw} x {ch} pixels"));
    }

    fn reset_crop(&mut self) {
        let Some(img) = &mut self.img else { return };
        img.crop = None;
        img.work = imgproc::fit_working(&img.original);
        img.analysis = imgproc::analyse(&img.work);
        self.cropping = false;
        self.image_changed();
    }

    fn start_crop(&mut self) {
        let Some(img) = &self.img else { return };
        let full = imgproc::fit_working(&img.original);
        let t = Texture2D::from_rgba8(full.width() as u16, full.height() as u16, &imgproc::rgba(&full));
        t.set_filter(FilterMode::Linear);
        self.crop_tex = Some(t);
        self.crop_sel = Some(match img.crop {
            Some((x, y, w, h)) => {
                let (ow, oh) = (img.original.width() as f32, img.original.height() as f32);
                Rect::new(x as f32 / ow, y as f32 / oh, w as f32 / ow, h as f32 / oh)
            }
            None => Rect::new(0.1, 0.1, 0.8, 0.8),
        });
        self.cropping = true;
    }

    /// "Open picture": the native file dialog; in the web build the browser's chooser, whose
    /// picture arrives a few frames later (main loop, `web::take_picture`). There a click on
    /// the button itself normally reaches the browser's file input (`Ui::picture_button`).
    fn open_dialog(&mut self) {
        #[cfg(target_arch = "wasm32")]
        crate::web::open_picture();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"])
            .set_title("Open a picture")
            .pick_file()
        {
            self.open_path(&path);
        }
    }

    pub fn open_path(&mut self, path: &std::path::Path) {
        match imgproc::load_path(path) {
            Ok(img) => {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.set_image(name.clone(), img);
                self.add_to_recent(path, &name);
            }
            Err(e) => self.say(format!("Could not open that picture: {e}")),
        }
    }

    /// Puts the picture just opened at the top of the recent list (under `key`) and restores
    /// the settings it had last time.
    fn add_to_recent(&mut self, key: &std::path::Path, name: &str) {
        self.picture_path = self.recent.add(key);
        let saved = self.picture_path.as_deref().and_then(|k| self.recent.settings(k)).cloned();
        match saved {
            Some(s) => {
                self.apply_settings(&s);
                self.say(format!("Opened {name} with its settings from last time"));
            }
            None => self.say(format!("Opened {name}")),
        }
        self.save_recent();
    }

    /// Menu of recently opened pictures (each with a × that removes it), then "Clear recent
    /// list"; with how many of the items can be removed.
    fn recent_menu_items(&self) -> (Vec<String>, usize) {
        if self.recent.pictures.is_empty() {
            return (vec!["No pictures opened yet".into()], 0);
        }
        let mut items: Vec<String> = self.recent.shown().iter().map(|p| recent::menu_label(&p.path)).collect();
        let removable = items.len();
        items.push("Clear recent list".into());
        (items, removable)
    }

    fn recent_menu_pick(&mut self, ui: &mut Ui, pick: Pick) {
        let i = match pick {
            Pick::Item(i) => i,
            Pick::Remove(i) => return self.remove_recent(ui, i),
        };
        let shown = self.recent.shown().len();
        if let Some(path) = self.recent.shown().get(i).map(|p| p.path.clone()) {
            // the web app's copy arrives a few frames later, like a chosen picture
            #[cfg(target_arch = "wasm32")]
            {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                crate::web::load_picture(&path.to_string_lossy(), &name);
            }
            #[cfg(not(target_arch = "wasm32"))]
            if path.is_file() {
                self.open_path(&path);
            } else {
                self.say(format!("{} is not there any more (moved, deleted or on a drive that is not connected)", path.display()));
            }
        } else if i == shown && i > 0 {
            self.recent.clear();
            self.save_recent();
            #[cfg(target_arch = "wasm32")]
            crate::web::keep_pictures(&[]);
            self.say("Recent list and saved picture settings cleared");
        }
    }

    /// Removes one picture (and its settings) from the recent list; the menu stays open.
    fn remove_recent(&mut self, ui: &mut Ui, i: usize) {
        let Some(key) = self.recent.remove(i) else { return };
        // the open picture is no longer in the list, so its settings are no longer kept
        if self.picture_path.as_deref() == Some(key.as_path()) {
            self.picture_path = None;
        }
        self.save_recent();
        #[cfg(target_arch = "wasm32")]
        crate::web::keep_pictures(&self.recent.shown_keys());
        let (items, removable) = self.recent_menu_items();
        ui.set_popup_items("open_recent", items, removable);
        let name = key.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        self.say(format!("Removed {name} from the recent list"));
    }

    /// Settings of the open picture, as kept in the recent list.
    pub fn picture_settings(&self) -> Option<PictureSettings> {
        let img = self.img.as_ref()?;
        Some(PictureSettings {
            crop: img.crop.map(|(x, y, w, h)| [x, y, w, h]),
            view: self.view,
            value_levels: self.value_levels,
            colour_levels: self.colour_levels,
            contour_detail: self.contour_detail,
            contours_on_top: self.contours_on_top,
            overlay: self.overlay,
            medium: self.medium,
            palette: self.current_palette().map(|p| p.name.clone()).unwrap_or_default(),
            max_paints: self.max_paints,
            allow_lighten: self.allow_lighten,
            mix_model: self.mix_model,
            method: self.method,
            picked: self.picked.as_ref().map(|p| [p.pos.x, p.pos.y]),
        })
    }

    /// Stores the open picture's settings in the recent list (and saves it) if they changed.
    fn remember_settings(&mut self) {
        let (Some(key), Some(s)) = (self.picture_path.clone(), self.picture_settings()) else { return };
        if self.recent.set_settings(&key, s) {
            self.save_recent();
        }
    }

    /// Restores a picture's settings right after it was opened. Values out of range are
    /// clamped, and a crop that no longer fits the picture (file changed) is dropped.
    fn apply_settings(&mut self, s: &PictureSettings) {
        if let (Some(img), Some([x, y, w, h])) = (&mut self.img, s.crop) {
            let (ow, oh) = img.original.dimensions();
            if w >= 8 && h >= 8 && x.saturating_add(w) <= ow && y.saturating_add(h) <= oh {
                img.crop = Some((x, y, w, h));
                img.work = imgproc::fit_working(&imgproc::crop(&img.original, (x, y, w, h)));
                img.analysis = imgproc::analyse(&img.work);
            }
        }
        self.view = s.view;
        self.value_levels = s.value_levels.clamp(2, 10);
        self.colour_levels = s.colour_levels.clamp(2, 24);
        self.contour_detail = s.contour_detail.clamp(imgproc::CONTOUR_DETAIL_MIN, imgproc::CONTOUR_DETAIL_MAX);
        self.contours_on_top = s.contours_on_top;
        self.overlay = s.overlay;
        self.overlay.cells = self.overlay.cells.clamp(2, 20);
        self.overlay.colour = self.overlay.colour.min(crate::grid::LINE_COLOURS.len() - 1);
        self.overlay.opacity = self.overlay.opacity.clamp(0.15, 1.0);
        self.max_paints = s.max_paints.clamp(1, 3);
        self.allow_lighten = s.allow_lighten;
        self.mix_model = s.mix_model;
        self.method = s.method;
        self.palette_idx = self.store.palettes.iter().position(|p| p.medium == s.medium && p.name == s.palette);
        self.select_medium(s.medium);
        self.image_changed();
        if let Some([x, y]) = s.picked {
            self.pick_at(vec2(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
        }
    }

    fn save_recent(&mut self) {
        if self.persist {
            if let Err(e) = recent::save(&self.recent) {
                self.say(format!("Could not save the recent pictures list: {e}"));
            }
        }
    }

    /// Opens a picture given as file contents (dropped in, or chosen in the web build).
    /// The web app also adds it to the recent list and keeps a copy of it for that.
    pub fn open_bytes(&mut self, name: &str, bytes: &[u8]) {
        let name = if name.is_empty() { "Picture".to_string() } else { name.to_string() };
        #[cfg(target_arch = "wasm32")]
        if bytes.is_empty() {
            // a picture from the recent list whose copy the browser no longer has
            self.say(format!("{name} is no longer kept in this browser; open it with Open picture..."));
            return;
        }
        match imgproc::load_bytes(bytes) {
            Ok(img) => {
                self.set_image(name.clone(), img);
                #[cfg(not(target_arch = "wasm32"))]
                self.say(format!("Opened {name}"));
                #[cfg(target_arch = "wasm32")]
                {
                    let key = recent::browser_key(&name, bytes);
                    self.add_to_recent(&key, &name);
                    crate::web::store_picture(&key.to_string_lossy(), bytes);
                    crate::web::keep_pictures(&self.recent.shown_keys());
                }
            }
            Err(e) => self.say(format!("Could not open that picture: {e}")),
        }
    }

    /// The mixer recipes are searched with: current palette, mixing model and method.
    pub fn recipe_mixer(&self) -> Option<Mixer> {
        self.current_palette().map(|p| Mixer::new(p, self.mix_model).with_method(self.method))
    }

    /// Bring derived data (levels, texture, mixer, recipes) up to date.
    pub fn update_derived(&mut self) {
        if self.mix_dirty {
            self.mix_dirty = false;
            self.mixer = self.recipe_mixer();
            self.recipes.clear();
            if let (Some(p), Some(m)) = (&mut self.picked, &self.mixer) {
                p.recipe = m.find(srgb8_to_lab(p.srgb), self.max_paints as usize, self.allow_lighten);
            }
        }
        let Some(img) = &self.img else { return };
        if self.values.is_none() {
            self.values = Some(imgproc::value_levels(&img.analysis, self.value_levels as usize));
            self.tex_dirty = true;
        }
        if self.colours.is_none() {
            self.colours = Some(imgproc::colour_levels(&img.work, &img.analysis, self.colour_levels as usize));
            self.recipes.clear();
            self.tex_dirty = true;
        }
        let want_contours = self.view == View::Contours || self.contours_on_top;
        if want_contours && self.contours.is_none() {
            let (w, h) = img.work.dimensions();
            self.contours = Some(imgproc::contours(&img.analysis, w, h, self.contour_detail));
            self.tex_dirty = true;
        }
        if self.tex_dirty {
            self.tex_dirty = false;
            self.display_rgba = match self.view {
                View::Original => imgproc::rgba(&img.work),
                View::Greyscale => imgproc::greyscale(&img.analysis),
                View::Values => {
                    let v = self.values.as_ref().unwrap();
                    let greys: Vec<[u8; 3]> = v
                        .lightness
                        .iter()
                        .map(|&l| {
                            let g = crate::colour::grey_for_lightness(l);
                            [g, g, g]
                        })
                        .collect();
                    imgproc::render_levels(&v.index, &greys, None)
                }
                View::Colours => {
                    let c = self.colours.as_ref().unwrap();
                    imgproc::render_levels(&c.index, &c.srgb, self.highlight)
                }
                View::Contours => imgproc::render_contours(self.contours.as_deref().unwrap_or(&[])),
            };
            if self.contours_on_top && self.view != View::Contours {
                if let Some(e) = &self.contours {
                    imgproc::overlay_contours(&mut self.display_rgba, e);
                }
            }
            let (w, h) = img.work.dimensions();
            match &self.texture {
                Some(t) if t.width() as u32 == w && t.height() as u32 == h => t.update_from_bytes(w, h, &self.display_rgba),
                _ => {
                    let t = Texture2D::from_rgba8(w as u16, h as u16, &self.display_rgba);
                    t.set_filter(FilterMode::Linear);
                    self.texture = Some(t);
                }
            }
        }
        // Recipes, a few per frame.
        if let (Some(c), Some(m)) = (&self.colours, &self.mixer) {
            if self.recipes.len() != c.lab.len() {
                self.recipes = vec![None; c.lab.len()];
            }
            let start = macroquad::miniquad::date::now();
            for i in 0..c.lab.len() {
                if self.recipes[i].is_none() {
                    self.recipes[i] = m.find(c.lab[i], self.max_paints as usize, self.allow_lighten);
                    if (macroquad::miniquad::date::now() - start) * 1000.0 > RECIPE_BUDGET_MS {
                        break;
                    }
                }
            }
        }
    }

    /// Pick the colour at a spot of the picture (0..1 coordinates) and find its recipe.
    pub fn pick_at(&mut self, n: Vec2) {
        self.update_derived();
        let Some(img) = &self.img else { return };
        let (iw, ih) = img.work.dimensions();
        let px = ((n.x * iw as f32) as u32).min(iw - 1);
        let py = ((n.y * ih as f32) as u32).min(ih - 1);
        let srgb = imgproc::sample_avg(&img.work, px, py, 2);
        let recipe = self.mixer.as_ref().and_then(|m| m.find(srgb8_to_lab(srgb), self.max_paints as usize, self.allow_lighten));
        self.picked = Some(Picked { pos: n, srgb, recipe });
    }

    pub fn recipes_done(&self) -> bool {
        self.colours.as_ref().is_some_and(|c| self.recipes.len() == c.lab.len() && self.recipes.iter().all(|r| r.is_some()))
    }

    fn save_if_needed(&mut self) {
        if self.dirty {
            self.dirty = false;
            if self.persist {
                if let Err(e) = palette::save_store(&self.store) {
                    self.say(format!("Could not save palettes: {e}"));
                }
            }
        }
    }

    // ---------- frame ----------

    /// The window is too narrow for the side-by-side panels (see `COMPACT_W`).
    pub fn compact() -> bool {
        screen_width() < COMPACT_W
    }

    /// Top of the screen's content: under the top bar, and the pane bar when compact.
    fn content_top() -> f32 {
        if Self::compact() { TOP_BAR + PANE_BAR } else { TOP_BAR }
    }

    pub fn frame(&mut self, ui: &mut Ui) {
        clear_background(BG);
        self.update_derived();
        let about_was_open = self.about_open;
        ui.set_covered(about_was_open);
        // Content scrolled up under the bars is covered by them and must not react.
        ui.set_clip_top(Self::content_top());
        match self.screen {
            Screen::Painting => self.painting(ui),
            Screen::Palettes => self.palettes(ui),
        }
        ui.set_clip_top(0.0);
        if Self::compact() {
            self.pane_bar(ui);
        }
        self.top_bar(ui);
        if Self::compact() {
            self.toast(ui);
        }
        ui.set_covered(false);
        self.about_dialog(ui, about_was_open);
        ui.end();
        self.save_if_needed();
        // A finished change (no button held, e.g. mid slider drag) is kept for this picture.
        if !ui.input.down {
            self.remember_settings();
        }
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let w = screen_width();
        draw_rectangle(0.0, 0.0, w, TOP_BAR, PANEL);
        draw_line(0.0, TOP_BAR, w, TOP_BAR, 1.0, BORDER);
        // The logo with the app's name and version (compact: the logo alone) opens the
        // "About" dialog, which checks for a newer version.
        let about = Rect::new(8.0, 6.0, if Self::compact() { 40.0 } else { LEFT_W - 16.0 }, TOP_BAR - 12.0);
        ui.mark("about", about);
        if ui.hovered(about) {
            ui::rrect(about, 5.0, ui::PANEL2);
        }
        if ui.clicked(about) {
            self.open_about();
        }
        ui.tooltip("about_tip", about, || vec![("About, and check for a newer version".to_string(), TEXT)]);
        // a tiny palette logo drawn from circles
        for (i, c) in [[200u8, 50, 60], [240, 170, 40], [40, 90, 170]].iter().enumerate() {
            draw_circle(22.0 + i as f32 * 9.0, 26.0 + if i == 1 { -5.0 } else { 2.0 }, 7.0, col(*c));
        }
        if Self::compact() {
            // no title; screens and "PDF" share the width
            let export = Rect::new(w - 72.0, 10.0, 62.0, 32.0);
            let tab_w = ((export.x - 58.0 - 12.0) / 2.0).min(125.0);
            let tab = |i: f32| Rect::new(50.0 + i * (tab_w + 6.0), 10.0, tab_w, 32.0);
            if ui.button_ex("tab_palettes", tab(0.0), "Palettes", self.screen == Screen::Palettes) {
                self.screen = Screen::Palettes;
                self.pane_scroll = 0.0;
            }
            if ui.button_ex("tab_painting", tab(1.0), "Painting", self.screen == Screen::Painting) {
                self.screen = Screen::Painting;
                self.pane_scroll = 0.0;
            }
            if ui.button("export_pdf", export, "PDF") {
                self.export_pdf();
            }
            return;
        }
        ui.text("PaletteCraft", 50.0, 25.0, 16.0, TEXT);
        ui.text(&format!("Version {}", crate::update::VERSION), 50.0, 41.0, 12.0, MUTED);
        let tabs = Rect::new(LEFT_W + 20.0, 10.0, 260.0, 32.0);
        if ui.button_ex("tab_palettes", Rect::new(tabs.x, tabs.y, 125.0, tabs.h), "1  Palettes", self.screen == Screen::Palettes) {
            self.screen = Screen::Palettes;
        }
        if ui.button_ex("tab_painting", Rect::new(tabs.x + 131.0, tabs.y, 125.0, tabs.h), "2  Painting", self.screen == Screen::Painting) {
            self.screen = Screen::Painting;
        }
        let export = Rect::new(w - 150.0, 10.0, 134.0, 32.0);
        if ui.button("export_pdf", export, "Export PDF...") {
            self.export_pdf();
        }
        let support = Rect::new(export.x - 160.0, 10.0, 150.0, 32.0);
        if ui.support_button("support", support, "Support on Ko-fi") {
            self.open_support();
        }
        if get_time() < self.status_until {
            let sw = ui.measure(&self.status, 15.0);
            ui.text(&self.status, (support.x - sw - 16.0).max(tabs.x + 280.0), 32.0, 15.0, ui::ACCENT);
        }
    }

    /// "Support on Ko-fi": opens the author's Ko-fi page in the browser.
    fn open_support(&mut self) {
        match crate::links::open_url(crate::links::KOFI_URL) {
            Ok(()) => self.say("Thank you! Ko-fi opens in your browser"),
            Err(e) => self.say(format!("Could not open the browser: {e}. The page is {}", crate::links::KOFI_URL)),
        }
    }

    /// Compact layout: the panes of the current screen, one shown at a time.
    fn pane_bar(&mut self, ui: &mut Ui) {
        let w = screen_width();
        draw_rectangle(0.0, TOP_BAR, w, PANE_BAR, PANEL);
        draw_line(0.0, TOP_BAR + PANE_BAR, w, TOP_BAR + PANE_BAR, 1.0, BORDER);
        let (labels, current) = match self.screen {
            Screen::Painting => (["Picture", "Adjust", "Recipes"], &mut self.pane_painting),
            Screen::Palettes => (["Palettes", "Paints", "Edit & library"], &mut self.pane_palettes),
        };
        if let Some(i) = ui.segmented("pane", Rect::new(8.0, TOP_BAR + 6.0, w - 13.0, PANE_BAR - 12.0), &labels, *current) {
            *current = i;
            self.pane_scroll = 0.0;
        }
    }

    /// Compact layout: the status message, at the bottom (no room for it in the top bar).
    fn toast(&self, ui: &Ui) {
        if get_time() >= self.status_until || self.status.is_empty() {
            return;
        }
        let (w, h) = (screen_width(), screen_height());
        let tw = (ui.measure(&self.status, 15.0) + 24.0).min(w - 16.0);
        let r = Rect::new((w - tw) / 2.0, h - 54.0, tw, 34.0);
        draw_rectangle(r.x + 2.0, r.y + 3.0, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
        crate::ui::rrect(r, 8.0, BG);
        crate::ui::rrect_lines(r, 8.0, ui::ACCENT);
        ui.text_fit(&self.status, r.x + 12.0, r.y + 22.0, 15.0, TEXT, r.w - 24.0);
    }
}

fn corners(r: Rect) -> [Vec2; 4] {
    [vec2(r.x, r.y), vec2(r.x + r.w, r.y), vec2(r.x + r.w, r.y + r.h), vec2(r.x, r.y + r.h)]
}

/// Small sign inside a paint dot: empty ring = transparent, half disc = half-transparent, full = opaque.
fn transparency_mark(cx: f32, cy: f32, t: Transparency) {
    let c = Color::new(1.0, 1.0, 1.0, 0.85);
    let shadow = Color::new(0.0, 0.0, 0.0, 0.5);
    draw_circle_lines(cx, cy, 4.5, 2.5, shadow);
    match t {
        Transparency::Transparent => draw_circle_lines(cx, cy, 4.0, 1.2, c),
        Transparency::HalfTransparent => {
            draw_circle_lines(cx, cy, 4.0, 1.2, c);
            draw_rectangle(cx - 4.0, cy, 8.0, 3.6, c);
        }
        Transparency::Opaque => draw_circle(cx, cy, 4.0, c),
    }
}

fn match_colour(de: f32) -> Color {
    if de < MATCH_GOOD {
        GOOD
    } else if de < MATCH_CLOSE {
        WARN
    } else {
        BAD
    }
}

/// The match words with their dE range and what they mean for the painter, best first
/// (explained in the app's pop-up and in the PDF legend).
fn match_bands() -> [(&'static str, String, &'static str); 4] {
    [
        ("excellent", format!("below {MATCH_EXCELLENT}"), "hard to tell apart"),
        ("good", format!("{MATCH_EXCELLENT} to {MATCH_GOOD}"), "small difference, seen side by side"),
        ("close", format!("{MATCH_GOOD} to {MATCH_CLOSE}"), "clearly different; adjust the mix by eye"),
        ("rough", format!("{MATCH_CLOSE} or more"), "your paints cannot reach this colour well"),
    ]
}

/// Hover help for the match word and its dE number; the band of this recipe is highlighted.
fn match_tip(de: f32) -> Vec<(String, Color)> {
    let here = match_label(de);
    let mut lines = vec![
        (format!("Colour match: {here}  (dE {de:.1})"), TEXT),
        ("dE (delta E) is how far the mixed colour is from the picture's".into(), MUTED),
        ("colour (the two swatches), measured in Lab colour space (CIE76).".into(), MUTED),
        ("0 means the same colour; about 2.3 is the smallest difference".into(), MUTED),
        ("most people can see side by side. Smaller is better.".into(), MUTED),
        (String::new(), MUTED),
    ];
    for ((word, range, meaning), band_colour) in match_bands().into_iter().zip([GOOD, GOOD, WARN, BAD]) {
        let colour = if word == here { band_colour } else { MUTED };
        lines.push((format!("{word}\t{range}\t{meaning}"), colour));
    }
    lines.push((String::new(), MUTED));
    lines.push(("The mixed colour comes from a paint-mixing model (Kubelka-Munk);".into(), MUTED));
    lines.push(("real paints, paper and brushwork can shift it a little.".into(), MUTED));
    lines
}

/// Recipe for a narrow card: the paints and parts broken over up to two lines where they
/// do not fit, then how much to lighten and how well it matches. Returns the lines used.
fn recipe_text_wrapped(ui: &mut Ui, id: &str, m: &Mixer, rc: &Recipe, x: f32, y: f32, w: f32) -> usize {
    // mixture: "3 Cobalt Blue + 1 Crimson"; glazes: "Cobalt Blue, then Crimson"
    let (pieces, sep) = m.part_pieces(rc);
    let mut lines = vec![String::new()];
    for (k, piece) in pieces.iter().enumerate() {
        let n = lines.len();
        let last = &mut lines[n - 1];
        let joined = if k == 0 { piece.to_string() } else { format!("{last}{sep}{piece}") };
        if k > 0 && ui.measure(&joined, 15.0) > w && n < 2 {
            last.push_str(sep.trim_end());
            lines.push(piece.to_string());
        } else {
            *last = joined;
        }
    }
    for (k, l) in lines.iter().enumerate() {
        ui.text_fit(l, x, y + 17.0 * k as f32, 15.0, TEXT, w);
    }
    let ly = y + 17.0 * lines.len() as f32;
    let lighten = m.describe_lighten(rc);
    // the dE number stays in the pop-up when the line is too short for it
    let mut ml = format!("{}  (dE {:.1})", match_label(rc.de), rc.de);
    if ui.measure(&lighten, 13.0) + ui.measure(&ml, 12.0) + 10.0 > w {
        ml = match_label(rc.de).to_string();
    }
    let mw = ui.measure(&ml, 12.0);
    ui.text_fit(&lighten, x, ly, 13.0, MUTED, w - mw - 10.0);
    ui.text(&ml, x + w - mw, ly, 12.0, match_colour(rc.de));
    let de = rc.de;
    ui.tooltip(id, Rect::new(x + w - mw - 3.0, ly - 13.0, mw + 6.0, 17.0), || match_tip(de));
    lines.len() + 1
}

/// Whether `recipe_text` shows `rc` whole in width `w`; if not, use `recipe_text_wrapped`.
fn recipe_fits(ui: &Ui, m: &Mixer, rc: &Recipe, w: f32) -> bool {
    let ml = format!("{}  (dE {:.1})", match_label(rc.de), rc.de);
    ui.measure(&m.describe_parts(rc), 15.0) <= w && ui.measure(&m.describe_lighten(rc), 13.0) + ui.measure(&ml, 12.0) + 10.0 <= w
}

/// Recipe in two lines: paints and parts, then how much to lighten and how well it matches.
/// `id` names the match label, which explains itself on hover.
fn recipe_text(ui: &mut Ui, id: &str, m: &Mixer, rc: &Recipe, x: f32, y: f32, w: f32) {
    ui.text_fit(&m.describe_parts(rc), x, y, 15.0, TEXT, w);
    let ml = format!("{}  (dE {:.1})", match_label(rc.de), rc.de);
    let mw = ui.measure(&ml, 12.0);
    ui.text_fit(&m.describe_lighten(rc), x, y + 17.0, 13.0, MUTED, w - mw - 10.0);
    ui.text(&ml, x + w - mw, y + 17.0, 12.0, match_colour(rc.de));
    let de = rc.de;
    ui.tooltip(id, Rect::new(x + w - mw - 3.0, y + 4.0, mw + 6.0, 17.0), || match_tip(de));
}
