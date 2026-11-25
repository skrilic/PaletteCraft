//! Recently opened pictures, newest first, with each picture's settings (crop, views,
//! overlay, paints), saved next to the palettes. The menu shows the newest `RECENT_MAX`;
//! settings are kept for the newest `REMEMBER_MAX`. The web app cannot open a file again by
//! its path, so it keeps a copy of each picture in the menu (see `browser_key`).

use crate::app::View;
use crate::grid::OverlaySettings;
use crate::km::{Method, MixModel};
use crate::palette::Medium;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How many pictures the "recently opened" menu shows.
pub const RECENT_MAX: usize = 10;
/// How many pictures keep their settings.
pub const REMEMBER_MAX: usize = 100;

/// What the painting screen looked like for one picture, restored when it is opened again.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct PictureSettings {
    /// Crop in pixels of the original picture: x, y, width, height.
    pub crop: Option<[u32; 4]>,
    pub view: View,
    pub value_levels: i32,
    pub colour_levels: i32,
    pub contour_detail: i32,
    pub contours_on_top: bool,
    pub overlay: OverlaySettings,
    pub medium: Medium,
    /// Palette name; when it is gone, the first palette of `medium` is used.
    pub palette: String,
    pub max_paints: i32,
    pub allow_lighten: bool,
    /// Mixing model (pictures saved before it existed get the default, spectral).
    #[serde(default)]
    pub mix_model: MixModel,
    /// Mix or glaze (pictures saved before it existed get the default, mix).
    #[serde(default)]
    pub method: Method,
    /// Clicked spot, 0..1 across the (cropped) picture.
    pub picked: Option<[f32; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecentPicture {
    pub path: PathBuf,
    #[serde(default)]
    pub settings: Option<PictureSettings>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Recent {
    pub version: u32,
    pub pictures: Vec<RecentPicture>,
}

impl Recent {
    /// Puts `path` at the top of the list (once), keeping its settings, and returns the key
    /// it is stored under (the full path), or `None` when it cannot be saved.
    pub fn add(&mut self, path: &Path) -> Option<PathBuf> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        // JSON keeps text only; a path that is not valid Unicode cannot be saved.
        path.to_str()?;
        self.version = 2;
        let entry = match self.pictures.iter().position(|p| p.path == path) {
            Some(i) => self.pictures.remove(i),
            None => RecentPicture { path: path.clone(), settings: None },
        };
        self.pictures.insert(0, entry);
        self.pictures.truncate(REMEMBER_MAX);
        Some(path)
    }

    /// Keys of the pictures in the menu, as text (the web app keeps a copy of these).
    #[cfg(target_arch = "wasm32")]
    pub fn shown_keys(&self) -> Vec<String> {
        self.shown().iter().filter_map(|p| p.path.to_str().map(str::to_string)).collect()
    }

    /// The pictures shown in the menu, newest first.
    pub fn shown(&self) -> &[RecentPicture] {
        &self.pictures[..self.pictures.len().min(RECENT_MAX)]
    }

    pub fn settings(&self, key: &Path) -> Option<&PictureSettings> {
        self.pictures.iter().find(|p| p.path == key).and_then(|p| p.settings.as_ref())
    }

    /// Stores the settings of a picture in the list; returns whether anything changed.
    pub fn set_settings(&mut self, key: &Path, settings: PictureSettings) -> bool {
        match self.pictures.iter_mut().find(|p| p.path == key) {
            Some(p) if p.settings.as_ref() != Some(&settings) => {
                p.settings = Some(settings);
                true
            }
            _ => false,
        }
    }

    /// Forgets the picture shown at `i` in the menu, with its settings; returns its key.
    pub fn remove(&mut self, i: usize) -> Option<PathBuf> {
        (i < self.shown().len()).then(|| self.pictures.remove(i).path)
    }

    /// Forgets all pictures and their settings.
    pub fn clear(&mut self) {
        self.pictures.clear();
    }
}

/// Key in the recent list of a picture opened in the web app: "browser-<hash>/<file name>",
/// the hash (64-bit FNV-1a) of its contents telling apart pictures of the same name.
#[cfg(any(target_arch = "wasm32", test))]
pub fn browser_key(name: &str, bytes: &[u8]) -> PathBuf {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    PathBuf::from(format!("browser-{hash:016x}")).join(name.replace(['/', '\\'], "_"))
}

/// Menu text for a recent picture: file name, a tab, then where it is
/// (the last two folders, or "not found" when the file is gone). The web app shows the
/// name only: it keeps its own copy of the picture.
pub fn menu_label(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string_lossy().to_string());
    if cfg!(target_arch = "wasm32") {
        return name;
    }
    if !path.is_file() {
        return format!("{name}\tnot found");
    }
    let folders: Vec<String> = path
        .parent()
        .map(|d| d.components().filter_map(|c| c.as_os_str().to_str()).filter(|c| *c != "/" && *c != "\\").map(str::to_string).collect())
        .unwrap_or_default();
    let tail = folders.len().saturating_sub(2);
    let mut place = folders[tail..].join("/");
    if tail > 0 {
        place = format!(".../{place}");
    }
    format!("{name}\t{place}")
}

/// Name of the saved list (see `storage`).
const RECENT_FILE: &str = "recent.json";

/// Saved list, or an empty one when there is none (or it cannot be read).
pub fn load() -> Recent {
    crate::storage::read(RECENT_FILE).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(recent: &Recent) -> Result<(), String> {
    let text = serde_json::to_string_pretty(recent).map_err(|e| e.to_string())?;
    crate::storage::write(RECENT_FILE, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Overlay;

    fn pic(i: usize) -> PathBuf {
        PathBuf::from(format!("/no/such/folder/pic{i}.png"))
    }

    fn settings() -> PictureSettings {
        PictureSettings {
            crop: Some([10, 20, 300, 200]),
            view: View::Values,
            value_levels: 3,
            colour_levels: 12,
            contour_detail: 7,
            contours_on_top: true,
            overlay: OverlaySettings { kind: Overlay::Grid, cells: 8, diagonals: true, flip_x: false, flip_y: true, colour: 2, opacity: 0.5 },
            medium: Medium::Oil,
            palette: "Oil 1".into(),
            max_paints: 2,
            allow_lighten: false,
            mix_model: MixModel::Rgb,
            method: Method::Glaze,
            picked: Some([0.25, 0.75]),
        }
    }

    #[test]
    fn newest_first_without_duplicates_and_capped() {
        let mut r = Recent::default();
        for i in 0..(REMEMBER_MAX + 3) {
            r.add(&pic(i));
        }
        r.add(&pic(5));
        assert_eq!(r.pictures.len(), REMEMBER_MAX);
        assert_eq!(r.shown().len(), RECENT_MAX);
        assert_eq!(r.pictures[0].path, pic(5));
        assert_eq!(r.pictures.iter().filter(|p| p.path == pic(5)).count(), 1);
        assert_eq!(r.pictures[1].path, pic(REMEMBER_MAX + 2));
    }

    #[test]
    fn settings_follow_the_picture_and_survive_saving() {
        let mut r = Recent::default();
        let key = r.add(&pic(1)).unwrap();
        r.add(&pic(2));
        assert!(r.set_settings(&key, settings()));
        assert!(!r.set_settings(&key, settings()), "same settings again is no change");
        assert!(!r.set_settings(&pic(99), settings()), "pictures not in the list keep nothing");
        // opening it again moves it to the top with its settings
        r.add(&pic(1));
        assert_eq!(r.pictures[0].path, pic(1));
        let back: Recent = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back.settings(&key), Some(&settings()));
        assert_eq!(back.settings(&pic(2)), None);
    }

    #[test]
    fn removing_one_forgets_it_and_its_settings() {
        let mut r = Recent::default();
        for i in 0..(RECENT_MAX + 1) {
            r.add(&pic(i));
        }
        let key = r.pictures[2].path.clone();
        assert!(r.set_settings(&key, settings()));
        assert_eq!(r.remove(2), Some(key.clone()));
        assert_eq!(r.settings(&key), None);
        assert!(r.pictures.iter().all(|p| p.path != key));
        // the oldest picture comes back into the menu
        assert_eq!(r.shown().len(), RECENT_MAX);
        assert_eq!(r.shown().last().map(|p| p.path.clone()), Some(pic(0)));
        assert_eq!(r.remove(RECENT_MAX), None, "only pictures shown in the menu");
    }

    #[test]
    fn menu_label_marks_missing_files() {
        assert_eq!(menu_label(Path::new("/no/such/folder/pic.png")), "pic.png\tnot found");
        let here = std::env::current_exe().unwrap();
        let label = menu_label(&here);
        assert!(label.contains('\t') && !label.ends_with("not found"), "{label}");
    }

    #[test]
    fn browser_keys_tell_pictures_apart() {
        let a = browser_key("IMG_1.jpeg", b"one picture");
        assert_eq!(a, browser_key("IMG_1.jpeg", b"one picture"));
        assert_ne!(a, browser_key("IMG_1.jpeg", b"another picture"));
        assert_eq!(a.file_name().unwrap(), "IMG_1.jpeg");
        assert_eq!(browser_key("a/b.png", b"x").file_name().unwrap(), "a_b.png");
        let mut r = Recent::default();
        assert_eq!(r.add(&a).as_deref(), Some(a.as_path()));
    }
}
