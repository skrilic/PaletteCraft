//! `--self-test`: drives the app with scripted mouse and keyboard input, the way a person would,
//! and checks from the app's own state that each part of the core loop works.

use crate::app::{App, Screen, View};
use crate::grid::Overlay;
use crate::km::{Lighten, MixModel};
use crate::palette::{Medium, Transparency};
use crate::recent::PictureSettings;
use crate::ui::{Input, Ui};
use macroquad::prelude::*;
use std::collections::{HashSet, VecDeque};

type CheckFn = Box<dyn FnMut(&App, &Ui, &mut Vec<StepResult>)>;
type UntilFn = Box<dyn Fn(&App) -> bool>;

enum Action {
    Wait(u32),
    WaitUntil(UntilFn, u32),
    Click(&'static str),
    ClickAt(&'static str, Vec2),
    /// Move the mouse over a control without pressing.
    Hover(&'static str),
    Drag(&'static str, Vec2, Vec2, u32),
    SliderTo(&'static str, f32),
    Type(&'static str),
    Backspace(u32),
    Check(CheckFn),
}

/// File name of the picture `main` writes to the temp folder and puts in the recent list.
pub const RECENT_PICTURE: &str = "palettecraft-selftest.png";

pub struct StepResult {
    pub step: String,
    pub passed: bool,
    pub detail: String,
}

pub struct SelfTest {
    queue: VecDeque<Action>,
    mouse: Vec2,
    /// Frames still to emit for the action being executed.
    pending: VecDeque<Input>,
    pub results: Vec<StepResult>,
    pub frames: u32,
}

fn res(r: &mut Vec<StepResult>, step: &str, passed: bool, detail: String) {
    r.push(StepResult { step: step.into(), passed, detail });
}

fn centre(ui: &Ui, id: &str) -> Option<Vec2> {
    ui.rects.get(id).map(|r| r.center())
}

impl SelfTest {
    pub fn new() -> Self {
        use Action::*;
        let mut q: VecDeque<Action> = VecDeque::new();
        let start_dims = std::rc::Rc::new(std::cell::Cell::new((0u32, 0u32)));
        q.push_back(Wait(6));

        // Value study: greyscale with the chosen number of levels.
        q.push_back(Click("view_values"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let total: f32 = app.values.as_ref().map(|v| v.share.iter().sum()).unwrap_or(0.0);
            res(r, "value_shares", (total - 1.0).abs() < 0.01, format!("value box percentages add up to {:.1}% of the picture", total * 100.0));
            let greys: HashSet<u8> = app.display_rgba.chunks(4).map(|p| p[0]).collect();
            let grey_only = app.display_rgba.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2]);
            let ok = app.view == View::Values && grey_only && greys.len() == app.value_levels as usize;
            res(r, "values", ok, format!("value view shows {} grey levels (asked for {})", greys.len(), app.value_levels));
        })));

        // Move: drag a crop frame on the picture and apply it.
        let d0 = start_dims.clone();
        q.push_back(Check(Box::new(move |app, _ui, _r| {
            if let Some(img) = &app.img {
                d0.set(img.work.dimensions());
            }
        })));
        q.push_back(Click("crop_start"));
        q.push_back(Wait(2));
        // The crop frame starts at 10%..90%; pull its bottom-right handle in to 70%.
        q.push_back(Drag("canvas_image", vec2(0.9, 0.9), vec2(0.7, 0.7), 14));
        q.push_back(Wait(1));
        q.push_back(Click("crop_apply"));
        q.push_back(Wait(3));
        let d1 = start_dims.clone();
        q.push_back(Check(Box::new(move |app, _ui, r| {
            let (w0, h0) = d1.get();
            let Some(img) = &app.img else {
                res(r, "move", false, "no picture loaded".into());
                return;
            };
            let (w1, h1) = img.work.dimensions();
            let crop = img.crop.unwrap_or((0, 0, 0, 0));
            let fx = crop.2 as f32 / img.original.width() as f32;
            let fy = crop.3 as f32 / img.original.height() as f32;
            let ok = img.crop.is_some() && (fx - 0.6).abs() < 0.05 && (fy - 0.6).abs() < 0.05 && !app.cropping;
            res(
                r,
                "move",
                ok,
                format!("dragged a crop frame over the picture: {w0}x{h0} became {w1}x{h1} ({:.0}% x {:.0}% of the original)", fx * 100.0, fy * 100.0),
            );
        })));

        // Act: colour levels and Kubelka-Munk recipes for each of them.
        q.push_back(SliderTo("colour_levels", (10.0 - 2.0) / (24.0 - 2.0)));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let n = app.colours.as_ref().map(|c| c.lab.len()).unwrap_or(0);
            let des: Vec<f32> = app.recipes.iter().flatten().map(|rc| rc.de).collect();
            let mean = des.iter().sum::<f32>() / des.len().max(1) as f32;
            let ok = app.view == View::Colours && n == 10 && des.len() == n && mean < 15.0;
            res(r, "act", ok, format!("{n} colour levels, {} Kubelka-Munk recipes from the palette, mean colour difference dE {:.1}", des.len(), mean));
            // Physical sanity: in the starter watercolor palette blue + yellow mixes to a green.
            if let Some(m) = &app.mixer {
                let blue = app.current_palette().and_then(|p| p.paints.iter().position(|x| x.name == "Cobalt Blue"));
                let yellow = app.current_palette().and_then(|p| p.paints.iter().position(|x| x.name == "Lemon Yellow"));
                if let (Some(b), Some(y)) = (blue, yellow) {
                    let mix = m.mix_srgb(&[(b, 1), (y, 1)], Lighten::None);
                    let h = crate::colour::hue_deg(crate::colour::srgb8_to_lab(mix));
                    res(r, "mixing", (100.0..200.0).contains(&h), format!("Cobalt Blue + Lemon Yellow mix to {:?}, hue {:.0} degrees (green)", mix, h));
                }
            }
        })));

        // Mixing model: recipes are spectral by default; switch to RGB and back.
        q.push_back(Check(Box::new(|app, _ui, r| {
            let model = app.mixer.as_ref().map(|m| m.model);
            res(r, "spectral_default", model == Some(MixModel::Spectral), format!("recipes are worked out with {:?} Kubelka-Munk", model));
        })));
        q.push_back(Hover("mix_model_tip"));
        q.push_back(Wait(1));
        q.push_back(Check(Box::new(|_app, ui, r| {
            let tip = ui.tooltip_text().unwrap_or_default();
            let ok = tip.contains("Spectral:") && tip.contains("31 wavelengths") && tip.contains("RGB:");
            res(r, "mix_model_tooltip", ok, format!("hovering the mixing model explains it: \"{}\"", tip.lines().nth(1).unwrap_or("no tooltip")));
        })));
        q.push_back(Click("mix_model_1"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.mix_model == MixModel::Rgb && app.mixer.as_ref().is_some_and(|m| m.model == MixModel::Rgb) && app.recipes_done();
            let mean = app.recipes.iter().flatten().map(|rc| rc.de).sum::<f32>() / app.recipes.len().max(1) as f32;
            res(r, "mix_model_rgb", ok, format!("RGB model chosen: {} recipes worked out again, mean dE {:.1}", app.recipes.len(), mean));
        })));
        q.push_back(Click("mix_model_0"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let mean = app.recipes.iter().flatten().map(|rc| rc.de).sum::<f32>() / app.recipes.len().max(1) as f32;
            // Spectral mixing neutralises complementaries: Crimson + Phthalo Green is a grey.
            let mut detail = format!("spectral model chosen again: {} recipes, mean dE {:.1}", app.recipes.len(), mean);
            let mut ok = app.mixer.as_ref().is_some_and(|m| m.model == MixModel::Spectral) && app.recipes_done();
            if let (Some(m), Some(p)) = (&app.mixer, app.current_palette()) {
                let a = p.paints.iter().position(|x| x.name == "Crimson");
                let b = p.paints.iter().position(|x| x.name == "Phthalo Green");
                if let (Some(a), Some(b)) = (a, b) {
                    let mix = m.mix_srgb(&[(a, 1), (b, 1)], Lighten::None);
                    let lab = crate::colour::srgb8_to_lab(mix);
                    let chroma = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
                    ok &= chroma < 6.0;
                    detail += &format!("; Crimson + Phthalo Green mix to {mix:?}, a near-neutral grey (chroma {chroma:.1})");
                }
            }
            res(r, "mix_model_spectral", ok, detail);
        })));

        // Glazing: recipes become layers (a first wash, then transparent glazes); then back to mixing.
        q.push_back(Hover("method_tip"));
        q.push_back(Wait(1));
        q.push_back(Check(Box::new(|_app, ui, r| {
            let tip = ui.tooltip_text().unwrap_or_default();
            let ok = ui.hud.contains("method") && tip.contains("Mix:") && tip.contains("Glaze:") && tip.contains("first wash");
            res(r, "method_tooltip", ok, format!("the Mix / Glaze switch is drawn and explains itself: \"{}\"", tip.lines().nth(2).unwrap_or("no tooltip")));
        })));
        q.push_back(Click("method_1"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let (Some(m), Some(p)) = (&app.mixer, app.current_palette()) else {
                res(r, "glaze", false, "no palette or mixer".into());
                return;
            };
            let recipes: Vec<_> = app.recipes.iter().flatten().collect();
            let glazed: Vec<_> = recipes.iter().filter(|rc| !rc.glazes.is_empty()).collect();
            let all_transparent = glazed.iter().all(|rc| rc.glazes.iter().all(|&(g, _)| p.paints[g].transparency != Transparency::Opaque));
            let within = recipes.iter().all(|rc| rc.parts.len() + rc.glazes.len() <= app.max_paints as usize);
            let consistent = recipes.iter().all(|rc| m.stages(rc).last() == Some(&rc.srgb));
            let example = glazed.first().map(|rc| format!("{} ({})", m.describe_parts(rc), m.describe_lighten(rc))).unwrap_or_default();
            let mean = recipes.iter().map(|rc| rc.de).sum::<f32>() / recipes.len().max(1) as f32;
            let ok = app.method == crate::km::Method::Glaze
                && m.method == crate::km::Method::Glaze
                && app.recipes_done()
                && !glazed.is_empty()
                && all_transparent
                && within
                && consistent
                && example.contains(", then ");
            res(
                r,
                "glaze",
                ok,
                format!(
                    "Glaze chosen: {} recipes, {} with glazes (only transparent or half-transparent paints, at most {} paints, colour = its layers), mean dE {mean:.1}; e.g. {example}",
                    recipes.len(),
                    glazed.len(),
                    app.max_paints
                ),
            );
        })));
        q.push_back(Click("method_0"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.method == crate::km::Method::Mix && app.recipes_done() && app.recipes.iter().flatten().all(|rc| rc.glazes.is_empty());
            res(r, "glaze_off", ok, "Mix chosen again: every recipe is a mixture, no glazes".into());
        })));

        // Pick a spot on the original picture.
        q.push_back(Click("view_original"));
        q.push_back(Wait(2));
        q.push_back(ClickAt("canvas_image", vec2(0.5, 0.45)));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.picked.as_ref().is_some_and(|p| p.recipe.is_some());
            let detail = match &app.picked {
                Some(p) => format!(
                    "clicked spot {:?} got a recipe: {}",
                    p.srgb,
                    p.recipe.as_ref().zip(app.mixer.as_ref()).map(|(rc, m)| m.describe_parts(rc)).unwrap_or_default()
                ),
                None => "no spot picked".into(),
            };
            res(r, "pick", ok, detail);
        })));
        // Hovering the match label ("good (dE 4.2)") explains what the words and dE mean.
        q.push_back(Hover("picked_match"));
        q.push_back(Wait(1));
        q.push_back(Check(Box::new(|app, ui, r| {
            let tip = ui.tooltip_text().unwrap_or_default();
            let label = app.picked.as_ref().and_then(|p| p.recipe.as_ref()).map(|rc| crate::km::match_label(rc.de)).unwrap_or("-");
            let ok = ui.hud.contains("picked_match")
                && tip.contains(&format!("Colour match: {label}"))
                && ["excellent", "good", "close", "rough", "delta E"].iter().all(|w| tip.contains(w));
            let first = tip.lines().next().unwrap_or("no tooltip");
            res(r, "match_tooltip", ok, format!("hovering the clicked spot's match label shows the dE explanation: \"{first}\""));
        })));

        // Overlays.
        q.push_back(Click("overlay_fib"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.overlay.kind == Overlay::Fibonacci && app.overlay_segments > 20;
            res(r, "overlay_fibonacci", ok, format!("Fibonacci spiral drawn with {} line segments", app.overlay_segments));
        })));
        q.push_back(Click("overlay_golden"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.overlay.kind == Overlay::Golden && app.overlay_segments == 4;
            res(r, "overlay_golden", ok, format!("golden ratio grid drawn with {} lines at 38.2% and 61.8%", app.overlay_segments));
        })));
        q.push_back(Click("overlay_grid"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.overlay.kind == Overlay::Grid && app.overlay_segments >= 4;
            res(r, "overlay_grid", ok, format!("grid drawn with {} lines", app.overlay_segments));
        })));
        q.push_back(Click("overlay_thirds"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let need = ["value_scale", "value_scale_caption", "value_levels_label", "colour_levels_label", "palette", "picked", "recipes", "overlay"];
            let missing: Vec<&str> = need.iter().copied().filter(|n| !ui.hud.contains(*n)).collect();
            let ok = missing.is_empty() && app.overlay.kind == Overlay::Thirds;
            let detail = if ok {
                "value scale, level counts, palette, clicked-spot recipe, recipe list and rule-of-thirds overlay are drawn".into()
            } else {
                format!("missing on screen: {}", missing.join(", "))
            };
            res(r, "hud", ok, detail);
        })));

        // Contours: line drawing of the picture, then the same lines over the original.
        q.push_back(Click("view_contours"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let share = app.contours.as_ref().map(|e| e.iter().filter(|&&b| b).count() as f32 / e.len().max(1) as f32).unwrap_or(0.0);
            let tones: HashSet<[u8; 3]> = app.display_rgba.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
            let ok = app.view == View::Contours && (0.005..0.3).contains(&share) && tones.len() == 2 && ui.hud.contains("contour_detail_label");
            res(r, "contours", ok, format!("contour view draws lines on paper: {:.1}% of the pixels are lines (detail {})", share * 100.0, app.contour_detail));
        })));
        q.push_back(Click("contours_on_top"));
        q.push_back(Click("view_original"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let Some((img, edge)) = app.img.as_ref().zip(app.contours.as_ref()) else {
                res(r, "contours_on_top", false, "no contour map".into());
                return;
            };
            let changed = img
                .work
                .pixels()
                .zip(app.display_rgba.chunks(4))
                .zip(edge)
                .filter(|((p, d), e)| **e && p.0 != [d[0], d[1], d[2]])
                .count();
            let ok = app.contours_on_top && app.view == View::Original && changed > 0;
            res(r, "contours_on_top", ok, format!("contour lines drawn over the original picture ({changed} pixels)"));
        })));

        // Export the reference brochure (saved to a temporary file in test mode).
        q.push_back(Check(Box::new(|app, _ui, _r| {
            if let Some(p) = &app.export_path_override {
                let _ = std::fs::remove_file(p);
            }
        })));
        q.push_back(Click("export_pdf"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let Some(path) = &app.export_path_override else {
                res(r, "export_pdf", false, "no export path".into());
                return;
            };
            let bytes = std::fs::read(path).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            let pages = text.matches("/Type /Page ").count();
            let images = text.matches("/Subtype /Image").count();
            let legend = ["(Match rating \\(dE\\))", "(excellent)", "(good)", "(close)", "(rough)"].iter().all(|s| text.contains(s));
            let ok = bytes.starts_with(b"%PDF-") && text.trim_end().ends_with("%%EOF") && pages >= 4 && images == 4 && legend;
            res(
                r,
                "export_pdf",
                ok,
                format!(
                    "Export PDF wrote a {} KB brochure with {pages} pages and {images} pictures (original, values, contours, colour levels){}",
                    bytes.len() / 1024,
                    if legend { ", ending with the match rating legend" } else { ", but the match rating legend is missing" }
                ),
            );
        })));
        // The brochure in glazing mode lists the layered recipes; then back to mixing.
        q.push_back(Click("method_1"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, _r| {
            if let Some(p) = &app.export_path_override {
                let _ = std::fs::remove_file(p);
            }
        })));
        q.push_back(Click("export_pdf"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let bytes = app.export_path_override.as_ref().and_then(|p| std::fs::read(p).ok()).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            let wanted = ["(Glazing recipes)", "(Method)", "(glazing: transparent layers over a first layer)", "(Layers)", ", then "];
            let missing: Vec<&str> = wanted.iter().copied().filter(|s| !text.contains(s)).collect();
            let ok = bytes.starts_with(b"%PDF-") && missing.is_empty();
            let detail = if missing.is_empty() {
                format!("Export PDF in Glaze mode: {} KB brochure with a \"Glazing recipes\" page and the method in its settings", bytes.len() / 1024)
            } else {
                format!("Export PDF in Glaze mode is missing {missing:?}")
            };
            res(r, "export_pdf_glaze", ok, detail);
        })));
        q.push_back(Click("method_0"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));

        // Palettes: create an acrylic palette and type in a paint.
        let count = std::rc::Rc::new(std::cell::Cell::new(0usize));
        q.push_back(Click("tab_palettes"));
        q.push_back(Wait(2));
        let c0 = count.clone();
        q.push_back(Check(Box::new(move |app, _ui, _r| c0.set(app.store.palettes.len()))));
        q.push_back(Click("new_palette_1"));
        q.push_back(Wait(2));
        q.push_back(Click("lib_row_0"));
        q.push_back(Wait(2));
        q.push_back(Click("add_paint"));
        q.push_back(Wait(2));
        q.push_back(Click("paint_name"));
        q.push_back(Backspace(9));
        q.push_back(Type("Crimson"));
        q.push_back(Click("paint_code"));
        q.push_back(Backspace(1));
        q.push_back(Type("PR206"));
        q.push_back(Click("paint_transparency_0"));
        q.push_back(Wait(2));
        let c1 = count.clone();
        q.push_back(Check(Box::new(move |app, _ui, r| {
            let new = app.store.palettes.last();
            let ok = app.screen == Screen::Palettes
                && app.store.palettes.len() == c1.get() + 1
                && new.is_some_and(|p| {
                    p.medium == Medium::Acrylic
                        && p.paints.len() == 2
                        && p.paints[1].name == "Crimson"
                        && p.paints[1].pigment == "PR206"
                        && p.paints[1].transparency == Transparency::Transparent
                });
            let detail = match new {
                Some(p) => format!(
                    "created \"{}\" ({}) with paints: {}",
                    p.name,
                    p.medium.label(),
                    p.paints.iter().map(|x| format!("{} {} {}", x.name, x.pigment, x.transparency.label())).collect::<Vec<_>>().join("; ")
                ),
                None => "no palette".into(),
            };
            res(r, "palette", ok, detail);
        })));

        // Colour library: save the paint to the artist's own library, edit it, add another,
        // filter by brand and use, group, add one to the palette, delete one.
        q.push_back(Click("save_to_library"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let c = app.store.library.first();
            let ok = app.store.library.len() == 1
                && app.lib_edit == Some(0)
                && ui.hud.contains("lib_colour_editor")
                && app.lib_unsaved == Some(0)
                && c.is_some_and(|c| c.name == "Crimson" && c.pigment == "PR206" && c.brand == crate::palette::DEFAULT_BRAND && c.uses == vec![Medium::Acrylic]);
            let detail = c.map(|c| format!("\"Save to my colour library\" kept {} {} (brand \"{}\", for {}) and opened it in the editor", c.name, c.pigment, c.brand, c.uses_label())).unwrap_or("nothing saved".into());
            res(r, "library_save", ok, detail);
        })));
        q.push_back(Click("libc_name"));
        q.push_back(Backspace(7));
        q.push_back(Type("Opera Rose"));
        q.push_back(Click("libc_code"));
        q.push_back(Backspace(5));
        q.push_back(Type("PR122"));
        q.push_back(Click("libc_brand"));
        q.push_back(Backspace(10));
        q.push_back(Type("Test Brand"));
        q.push_back(Click("libc_use_0"));
        q.push_back(Click("libc_transparency_1"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let c = &app.store.library[0];
            let palette_kept = app.store.palettes.last().is_some_and(|p| p.paints.get(1).is_some_and(|x| x.name == "Crimson"));
            let ok = c.name == "Opera Rose"
                && c.pigment == "PR122"
                && c.brand == "Test Brand"
                && c.uses == vec![Medium::Watercolor, Medium::Acrylic]
                && c.transparency == Transparency::HalfTransparent
                && palette_kept;
            res(
                r,
                "library_edit",
                ok,
                format!("edited to {} {} by \"{}\", {}, for {}; the palette's paint stays \"Crimson\" (palettes keep copies)", c.name, c.pigment, c.brand, c.transparency.label(), c.uses_label()),
            );
        })));
        q.push_back(Click("lib_new"));
        q.push_back(Wait(2));
        q.push_back(Click("libc_save"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let c = app.store.library.get(1);
            let ok = app.store.library.len() == 2
                && app.lib_edit.is_none()
                && app.lib_unsaved.is_none()
                && c.is_some_and(|c| c.brand == "Test Brand" && c.uses == vec![Medium::Acrylic]);
            let detail = c.map(|c| format!("\"+ New colour\" added \"{}\" (brand \"{}\" from the last edit, for {}); Save kept it and closed the editor", c.name, c.brand, c.uses_label())).unwrap_or("no new colour".into());
            res(r, "library_new", ok, detail);
        })));
        // "+ New colour" can be cancelled: Cancel and Save replace Delete and Done until saved
        q.push_back(Click("lib_new"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let ok = app.store.library.len() == 3
                && app.lib_unsaved == Some(2)
                && ui.rects.contains_key("libc_cancel")
                && ui.rects.contains_key("libc_save")
                && !ui.rects.contains_key("libc_delete");
            res(r, "library_new_offers_cancel", ok, "a colour just added shows Cancel and Save instead of Delete and Done".into());
        })));
        q.push_back(Click("libc_cancel"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let ok = app.store.library.len() == 2 && app.lib_edit.is_none() && app.lib_unsaved.is_none();
            res(r, "library_cancel", ok, format!("Cancel took the new colour out again ({} colours in your library)", app.store.library.len()));
        })));
        q.push_back(Click("lib_brand"));
        q.push_back(Wait(1));
        q.push_back(Click("lib_brand#1"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let rows = app.library_rows();
            let colours: Vec<_> = rows.iter().filter_map(|x| if let crate::app::LibRow::Colour { custom, colour } = x { Some((custom, colour)) } else { None }).collect();
            let ok = app.lib_brand.as_deref() == Some("Test Brand") && colours.len() == 2 && colours.iter().all(|(c, _)| c.is_some());
            let names: Vec<&str> = colours.iter().map(|(_, c)| c.name.as_str()).collect();
            res(r, "library_brand_filter", ok, format!("brand filter \"Test Brand\" lists {} colours: {}", colours.len(), names.join(", ")));
        })));
        q.push_back(Click("lib_brand"));
        q.push_back(Wait(1));
        q.push_back(Click("lib_brand#0"));
        q.push_back(Click("lib_use_3"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let rows = app.library_rows();
            let colours: Vec<_> = rows.iter().filter_map(|x| if let crate::app::LibRow::Colour { custom, colour } = x { Some((custom, colour)) } else { None }).collect();
            let ok = app.lib_brand.is_none()
                && app.lib_use == Some(Medium::Oil)
                && colours.iter().all(|(_, c)| c.has_use(Medium::Oil))
                && colours.iter().all(|(c, _)| c.is_none())
                && colours.len() == crate::palette::LIBRARY.len();
            res(r, "library_use_filter", ok, format!("use filter Oil lists {} colours, all for oil (your two watercolor/acrylic colours are hidden)", colours.len()));
        })));
        q.push_back(Click("lib_use_0"));
        q.push_back(Click("lib_group_1"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let headers: Vec<String> = app.library_rows().iter().filter_map(|x| if let crate::app::LibRow::Header(h, n) = x { Some(format!("{h} ({n})")) } else { None }).collect();
            let want = [format!("Acrylic (1)"), format!("Watercolor + Acrylic (1)"), format!("Any paint ({})", crate::palette::LIBRARY.len())];
            let ok = app.lib_group == crate::app::LibGroup::Use && headers == want;
            res(r, "library_group", ok, format!("grouped by use: {}", headers.join(", ")));
        })));
        q.push_back(Click("lib_group_0"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let headers: Vec<String> = app.library_rows().iter().filter_map(|x| if let crate::app::LibRow::Header(h, _) = x { Some(h.clone()) } else { None }).collect();
            // only the artist's own colours have an Edit button; general ones are fixed
            let ok = headers == ["Test Brand", "General"] && ui.rects.contains_key("lib_edit_0") && ui.rects.contains_key("lib_edit_1") && !ui.rects.contains_key("lib_edit_2") && ui.rects.contains_key("lib_row_2");
            res(r, "library_fixed_general", ok, format!("grouped by brand: {}; Edit only on your own colours, not on the general ones", headers.join(", ")));
        })));
        q.push_back(Click("lib_edit_0"));
        q.push_back(Wait(2));
        q.push_back(Click("libc_add"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let last = app.store.palettes.last().and_then(|p| p.paints.last());
            let ok = app.lib_edit == Some(0) && last.is_some_and(|p| p.name == "Opera Rose" && p.pigment == "PR122");
            res(r, "library_to_palette", ok, format!("Edit opened Opera Rose; \"Add to palette\" added {}", last.map(|p| p.title()).unwrap_or_default()));
        })));
        q.push_back(Click("libc_delete"));
        q.push_back(Wait(2));
        q.push_back(Click("libc_delete"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let kept = app.store.palettes.last().and_then(|p| p.paints.last()).is_some_and(|p| p.name == "Opera Rose");
            let ok = app.store.library.len() == 1 && app.store.library[0].name == "New colour" && app.lib_edit.is_none() && kept;
            res(r, "library_delete", ok, format!("Delete (clicked twice) removed Opera Rose from the library ({} left); the palette keeps its copy", app.store.library.len()));
        })));

        // Switch the painting to acrylic paints.
        q.push_back(Click("tab_painting"));
        q.push_back(Wait(2));
        q.push_back(Click("medium_1"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let pal = app.current_palette();
            let uses_white = app.recipes.iter().flatten().any(|rc| matches!(rc.lighten, Lighten::White(_)));
            let ok = app.medium == Medium::Acrylic && pal.is_some_and(|p| p.medium == Medium::Acrylic) && app.recipes_done();
            res(
                r,
                "medium",
                ok,
                format!(
                    "switched to acrylic: palette \"{}\", recipes recomputed{}",
                    pal.map(|p| p.name.as_str()).unwrap_or("-"),
                    if uses_white { ", lighter colours use white paint" } else { "" }
                ),
            );
        })));
        // Glazing with acrylic: an opaque first layer (one paint with white, or the white
        // ground), glazes of transparent paints only; then back to mixing.
        q.push_back(Click("method_1"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));
        q.push_back(Check(Box::new(|app, ui, r| {
            let (Some(m), Some(p)) = (&app.mixer, app.current_palette()) else {
                res(r, "glaze_acrylic", false, "no palette or mixer".into());
                return;
            };
            let recipes: Vec<_> = app.recipes.iter().flatten().collect();
            let glazed: Vec<_> = recipes.iter().filter(|rc| !rc.glazes.is_empty()).collect();
            let transparent_only = glazed.iter().all(|rc| rc.glazes.iter().all(|&(g, _)| p.paints[g].transparency != Transparency::Opaque && !p.paints[g].is_white()));
            let first_layers_ok = recipes.iter().all(|rc| rc.parts.len() <= 1 && rc.parts.iter().all(|&(i, _)| !p.paints[i].is_white()));
            let consistent = recipes.iter().all(|rc| m.stages(rc).last() == Some(&rc.srgb));
            let example = glazed.first().map(|rc| format!("{} ({})", m.describe_parts(rc), m.describe_lighten(rc))).unwrap_or_default();
            let ok = app.medium == Medium::Acrylic
                && m.medium == Medium::Acrylic
                && app.recipes_done()
                && (!m.can_glaze() || !glazed.is_empty())
                && transparent_only
                && first_layers_ok
                && consistent
                && ui.hud.contains("method");
            res(
                r,
                "glaze_acrylic",
                ok,
                format!(
                    "acrylic palette \"{}\" glazed: {} of {} recipes use glazes of transparent paints over an opaque first layer; e.g. {example}",
                    p.name,
                    glazed.len(),
                    recipes.len()
                ),
            );
        })));
        q.push_back(Click("method_0"));
        q.push_back(Wait(2));
        q.push_back(WaitUntil(Box::new(|app| app.recipes_done()), 900));

        // Recently opened: the arrow beside "Open picture..." lists the picture main put there.
        q.push_back(Click("open_recent"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|_app, ui, r| {
            let ok = ui.rects.contains_key("open_recent#0") && ui.rects.contains_key("open_recent#1") && !ui.rects.contains_key("open_recent#2");
            res(r, "recent_menu", ok, "the recent menu lists one picture and \"Clear recent list\"".into());
        })));
        q.push_back(Click("open_recent#0"));
        q.push_back(Wait(3));
        q.push_back(Check(Box::new(|app, _ui, r| {
            let name = app.img.as_ref().map(|i| i.name.as_str()).unwrap_or("-");
            let first = app.recent.pictures.first().and_then(|p| p.path.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let ok = name == RECENT_PICTURE && app.recent.pictures.len() == 1 && first == RECENT_PICTURE && !app.persist;
            res(r, "open_recent", ok, format!("picked \"{first}\" from the recent menu and it opened as \"{name}\" (list kept in memory only)"));
        })));

        // Settings per picture: change them here, switch to the sample picture, then reopen.
        q.push_back(Click("view_values"));
        q.push_back(SliderTo("value_levels", (3.0 - 2.0) / (10.0 - 2.0)));
        q.push_back(Click("overlay_grid"));
        q.push_back(Click("crop_start"));
        q.push_back(Wait(2));
        q.push_back(Drag("canvas_image", vec2(0.9, 0.9), vec2(0.6, 0.7), 14));
        q.push_back(Wait(1));
        q.push_back(Click("crop_apply"));
        q.push_back(Wait(3));
        q.push_back(ClickAt("canvas_image", vec2(0.4, 0.5)));
        q.push_back(Wait(2));
        let kept: std::rc::Rc<std::cell::RefCell<Option<PictureSettings>>> = Default::default();
        let k0 = kept.clone();
        q.push_back(Check(Box::new(move |app, _ui, r| {
            let now = app.picture_settings();
            let stored = app.picture_path.as_deref().and_then(|k| app.recent.settings(k)).cloned();
            let ok = now.is_some() && now == stored;
            res(r, "settings_kept", ok, "after each change the picture's settings are kept in the recent list".into());
            *k0.borrow_mut() = now;
        })));
        q.push_back(Click("sample_image"));
        q.push_back(Wait(2));
        q.push_back(Click("view_colours"));
        q.push_back(SliderTo("value_levels", 1.0));
        q.push_back(Click("overlay_none"));
        q.push_back(Wait(2));
        q.push_back(Click("open_recent"));
        q.push_back(Wait(2));
        q.push_back(Click("open_recent#0"));
        q.push_back(Wait(3));
        let k1 = kept.clone();
        q.push_back(Check(Box::new(move |app, _ui, r| {
            let want = k1.borrow().clone();
            let got = app.picture_settings();
            let crop = app.img.as_ref().and_then(|i| i.crop);
            let ok = want.is_some()
                && got == want
                && app.view == View::Values
                && app.value_levels == 3
                && app.overlay.kind == Overlay::Grid
                && crop.is_some()
                && app.picked.as_ref().is_some_and(|p| p.recipe.is_some());
            res(
                r,
                "settings_restored",
                ok,
                format!(
                    "reopened the picture with its own settings: {} view, {} value levels, {:?} overlay, crop {:?}, clicked spot {}",
                    app.view.label(),
                    app.value_levels,
                    app.overlay.kind,
                    crop,
                    if app.picked.is_some() { "restored" } else { "missing" }
                ),
            );
        })));

        // The × beside a picture in the recent menu removes it; the menu stays open.
        q.push_back(Click("open_recent"));
        q.push_back(Wait(2));
        q.push_back(Click("open_recent#0:remove"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let ok = app.recent.pictures.is_empty()
                && app.picture_path.is_none()
                && ui.rects.contains_key("open_recent#0")
                && !ui.rects.contains_key("open_recent#0:remove")
                && !ui.rects.contains_key("open_recent#1");
            res(r, "recent_remove", ok, "the × removed the picture from the recent list and the menu stayed open, now empty".into());
        })));
        q.push_back(Click("view_original"));
        q.push_back(Wait(2));

        // Close the picture: back to "No picture yet", nothing derived from it is kept.
        q.push_back(Check(Box::new(|app, ui, r| {
            let ok = app.img.is_some() && ui.rects.contains_key("close_image");
            res(r, "close_button", ok, "an open picture shows a Close button beside its name".into());
        })));
        q.push_back(Click("close_image"));
        q.push_back(Wait(2));
        q.push_back(Check(Box::new(|app, ui, r| {
            let ok = app.img.is_none()
                && app.values.is_none()
                && app.colours.is_none()
                && app.picked.is_none()
                && app.recipes.is_empty()
                && !app.cropping
                && !ui.rects.contains_key("close_image")
                && ui.rects.contains_key("open_image");
            res(r, "close_picture", ok, "Close closed the picture: no picture, levels, recipes or clicked spot left; Open picture... is still there".into());
        })));
        SelfTest { queue: q, mouse: vec2(5.0, 5.0), pending: VecDeque::new(), results: Vec::new(), frames: 0 }
    }

    fn base(&self) -> Input {
        Input { mouse: self.mouse, ..Default::default() }
    }

    /// Input for the next frame; `None` once the script is finished.
    pub fn input(&mut self, app: &App, ui: &Ui) -> Option<Input> {
        self.frames += 1;
        loop {
            if let Some(i) = self.pending.pop_front() {
                self.mouse = i.mouse;
                return Some(i);
            }
            let action = self.queue.pop_front()?;
            match action {
                Action::Wait(n) => {
                    for _ in 0..n {
                        self.pending.push_back(self.base());
                    }
                }
                Action::WaitUntil(f, max) => {
                    if !f(app) && max > 0 {
                        self.queue.push_front(Action::WaitUntil(f, max - 1));
                        return Some(self.base());
                    }
                }
                Action::Click(id) => match centre(ui, id) {
                    Some(p) => self.click(p),
                    None => self.results.push(StepResult { step: format!("click {id}"), passed: false, detail: "control not on screen".into() }),
                },
                Action::Hover(id) => match centre(ui, id) {
                    Some(p) => self.pending.push_back(Input { mouse: p, ..Default::default() }),
                    None => self.results.push(StepResult { step: format!("hover {id}"), passed: false, detail: "control not on screen".into() }),
                },
                Action::ClickAt(id, n) => {
                    if let Some(r) = ui.rects.get(id) {
                        self.click(r.point() + r.size() * n);
                    }
                }
                Action::SliderTo(id, t) => {
                    if let Some(r) = ui.rects.get(id) {
                        let p = vec2(r.x + 8.0 + (r.w - 16.0) * t, r.y + r.h / 2.0);
                        self.click(p);
                    }
                }
                Action::Drag(id, a, b, steps) => {
                    if let Some(r) = ui.rects.get(id) {
                        let pa = r.point() + r.size() * a;
                        let pb = r.point() + r.size() * b;
                        self.pending.push_back(Input { mouse: pa, ..Default::default() });
                        self.pending.push_back(Input { mouse: pa, down: true, pressed: true, ..Default::default() });
                        for k in 1..=steps {
                            let p = pa.lerp(pb, k as f32 / steps as f32);
                            self.pending.push_back(Input { mouse: p, down: true, ..Default::default() });
                        }
                        self.pending.push_back(Input { mouse: pb, ..Default::default() });
                    }
                }
                Action::Type(s) => {
                    self.pending.push_back(Input { mouse: self.mouse, chars: s.chars().collect(), ..Default::default() });
                }
                Action::Backspace(n) => {
                    for _ in 0..n {
                        self.pending.push_back(Input { mouse: self.mouse, backspace: true, ..Default::default() });
                    }
                }
                Action::Check(mut f) => f(app, ui, &mut self.results),
            }
        }
    }

    fn click(&mut self, p: Vec2) {
        self.pending.push_back(Input { mouse: p, ..Default::default() });
        self.pending.push_back(Input { mouse: p, down: true, pressed: true, ..Default::default() });
        self.pending.push_back(Input { mouse: p, ..Default::default() });
    }

    pub fn write(&self, path: &str) {
        let steps: Vec<serde_json::Value> = self
            .results
            .iter()
            .map(|s| serde_json::json!({"step": s.step, "passed": s.passed, "detail": s.detail}))
            .collect();
        let out = serde_json::json!({"version": 1, "steps": steps});
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&out).unwrap());
    }
}
