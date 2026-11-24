//! "Export PDF": a reference brochure with the picture, value study, contours,
//! the chosen palette and the colour mixes.

use super::*;
use crate::km::{Lighten, MATCH_CLOSE, MATCH_GOOD, Mixer};
use crate::pdf::{A4_H, A4_W, Doc, fit, wrap};
use std::path::{Path, PathBuf};

const MARGIN: f32 = 42.0;
const INK: [u8; 3] = [30, 30, 36];
const MUTED: [u8; 3] = [112, 112, 120];
const ACCENT: [u8; 3] = [196, 118, 38];
const RULE: [u8; 3] = [214, 208, 198];
const GOOD: [u8; 3] = [46, 140, 70];
const WARN: [u8; 3] = [196, 130, 20];
const BAD: [u8; 3] = [190, 60, 50];
/// Space kept free at the bottom of every page for the footer.
const FOOTER: f32 = 56.0;

fn rgba_to_rgb(rgba: &[u8], w: u32, h: u32) -> RgbImage {
    let data: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    RgbImage::from_raw(w, h, data).expect("image size")
}

/// Today's date as "18 November 2026" (UTC).
fn today() -> String {
    let secs = macroquad::miniquad::date::now().max(0.0) as u64;
    // civil-from-days (Howard Hinnant)
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

struct Pages {
    doc: Doc,
    y: f32,
    footer: String,
}

impl Pages {
    fn content_w(&self) -> f32 {
        self.doc.w - 2.0 * MARGIN
    }

    fn bottom(&self) -> f32 {
        self.doc.h - FOOTER
    }

    fn page(&mut self) {
        self.doc.new_page();
        let n = self.doc.page_count();
        let (w, h) = (self.doc.w, self.doc.h);
        self.doc.line(MARGIN, h - 40.0, w - MARGIN, h - 40.0, RULE, 0.6);
        let f = fit(&self.footer, 8.0, false, w - 2.0 * MARGIN - 60.0);
        self.doc.text(MARGIN, h - 26.0, 8.0, false, MUTED, &f);
        self.doc.text_right(w - MARGIN, h - 26.0, 8.0, false, MUTED, &format!("Page {n}"));
        self.y = MARGIN;
    }

    fn ensure(&mut self, h: f32) {
        if self.y + h > self.bottom() {
            self.page();
        }
    }

    fn heading(&mut self, title: &str, sub: &str) {
        self.ensure(70.0);
        self.doc.text(MARGIN, self.y + 18.0, 18.0, true, INK, title);
        self.y += 26.0;
        self.doc.rect(MARGIN, self.y, 36.0, 2.0, ACCENT);
        self.y += 10.0;
        for l in wrap(sub, 9.5, false, self.content_w()) {
            self.y += 12.0;
            self.doc.text(MARGIN, self.y, 9.5, false, MUTED, &l);
        }
        self.y += 12.0;
    }

    fn small_heading(&mut self, s: &str) {
        self.ensure(30.0);
        self.doc.text(MARGIN, self.y + 12.0, 11.0, true, INK, s);
        self.y += 20.0;
    }

    /// Picture centred across the page, as large as fits the box; returns its height.
    fn picture(&mut self, handle: usize, iw: u32, ih: u32, max_w: f32, max_h: f32, border: bool) -> f32 {
        let s = (max_w / iw as f32).min(max_h / ih as f32);
        let (w, h) = (iw as f32 * s, ih as f32 * s);
        let x = MARGIN + (self.content_w() - w) / 2.0;
        self.doc.image(handle, x, self.y, w, h);
        if border {
            self.doc.rect_stroke(x, self.y, w, h, RULE, 0.6);
        }
        self.y += h;
        h
    }
}

/// Paint dot with the transparency sign used in the app.
fn paint_dot(doc: &mut Doc, cx: f32, cy: f32, r: f32, c: [u8; 3], t: Transparency) {
    doc.circle(cx, cy, r, c);
    doc.circle_stroke(cx, cy, r, RULE, 0.5);
    let s = r * 0.42;
    match t {
        Transparency::Transparent => doc.circle_stroke(cx, cy, s, [255, 255, 255], 0.8),
        Transparency::HalfTransparent => {
            doc.circle_stroke(cx, cy, s, [255, 255, 255], 0.8);
            doc.rect(cx - s, cy, 2.0 * s, s * 0.9, [255, 255, 255]);
        }
        Transparency::Opaque => doc.circle(cx, cy, s, [255, 255, 255]),
    }
}

fn match_colour(de: f32) -> [u8; 3] {
    if de < MATCH_GOOD {
        GOOD
    } else if de < MATCH_CLOSE {
        WARN
    } else {
        BAD
    }
}

/// Legend for the match rating after the recipe list: what dE means and the four bands.
fn match_legend(p: &mut Pages) {
    let cw = p.content_w();
    let intro = wrap(
        "dE (delta E) is the colour difference between the PICTURE and MIX swatches, measured in Lab colour space (CIE76). \
         0 means the same colour; about 2.3 is the smallest difference most people can see side by side. Smaller is better.",
        9.0,
        false,
        cw,
    );
    let bands = match_bands();
    let row_h = 14.0;
    // heading, intro and table stay together on one page
    p.y += 14.0;
    p.ensure(20.0 + intro.len() as f32 * 12.0 + 18.0 + bands.len() as f32 * row_h);
    p.small_heading("Match rating (dE)");
    for l in &intro {
        p.y += 12.0;
        p.doc.text(MARGIN, p.y, 9.0, false, MUTED, l);
    }
    p.y += 18.0;
    for ((word, range, meaning), colour) in bands.iter().zip([GOOD, GOOD, WARN, BAD]) {
        p.doc.text(MARGIN, p.y, 9.5, true, colour, word);
        p.doc.text(MARGIN + 70.0, p.y, 9.5, false, INK, range);
        p.doc.text(MARGIN + 150.0, p.y, 9.5, false, INK, meaning);
        p.y += row_h;
    }
}

impl App {
    /// Button action: ask for a file name and folder, then save the brochure there.
    pub(super) fn export_pdf(&mut self) {
        let Some(img) = &self.img else {
            self.say("Open a picture first, then export the brochure");
            return;
        };
        if self.current_palette().is_none() {
            self.say("Choose or create a palette first, then export the brochure");
            return;
        }
        let stem = Path::new(&img.name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Picture".into());
        let file_name = format!("{stem} - reference.pdf");
        // The browser saves it as a download (to Downloads, or the share sheet on phones).
        #[cfg(target_arch = "wasm32")]
        if self.export_path_override.is_none() {
            match self.brochure_bytes() {
                Ok((bytes, pages)) => {
                    crate::web::save_file(&file_name, "application/pdf", &bytes);
                    self.say(format!("Saved \"{file_name}\" ({pages} pages)"));
                }
                Err(e) => self.say(format!("Could not make the PDF: {e}")),
            }
            return;
        }
        let path = match &self.export_path_override {
            Some(p) => Some(p.clone()),
            #[cfg(target_arch = "wasm32")]
            None => None,
            #[cfg(not(target_arch = "wasm32"))]
            None => {
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Save the reference brochure")
                    .add_filter("PDF document", &["pdf"])
                    .set_file_name(file_name);
                if let Some(dir) = &self.last_export_dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.save_file()
            }
        };
        let Some(mut path) = path else { return };
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            let mut s = path.into_os_string();
            s.push(".pdf");
            path = PathBuf::from(s);
        }
        match self.export_pdf_to(&path) {
            Ok(pages) => {
                self.last_export_dir = path.parent().map(Path::to_path_buf);
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.say(format!("Saved \"{name}\" ({pages} pages)"));
            }
            Err(e) => self.say(format!("Could not save the PDF: {e}")),
        }
    }

    /// Builds the brochure and writes it to `path`; returns the number of pages.
    pub fn export_pdf_to(&mut self, path: &Path) -> Result<usize, String> {
        let (bytes, pages) = self.brochure_bytes()?;
        std::fs::write(path, &bytes).map_err(|e| e.to_string())?;
        Ok(pages)
    }

    /// Finishes the derived data the brochure needs, then builds it: (PDF bytes, pages).
    fn brochure_bytes(&mut self) -> Result<(Vec<u8>, usize), String> {
        self.update_derived();
        // finish any recipes still being searched
        if let (Some(c), Some(m)) = (&self.colours, &self.mixer) {
            if self.recipes.len() != c.lab.len() {
                self.recipes = vec![None; c.lab.len()];
            }
            for i in 0..c.lab.len() {
                if self.recipes[i].is_none() {
                    self.recipes[i] = m.find(c.lab[i], self.max_paints as usize, self.allow_lighten);
                }
            }
        }
        if self.contours.is_none() {
            if let Some(img) = &self.img {
                let (w, h) = img.work.dimensions();
                self.contours = Some(imgproc::contours(&img.analysis, w, h, self.contour_detail));
            }
        }
        self.brochure()
    }

    fn brochure(&self) -> Result<(Vec<u8>, usize), String> {
        let img = self.img.as_ref().ok_or("no picture")?;
        let pal = self.current_palette().ok_or("no palette")?;
        let mixer = Mixer::new(pal, self.mix_model);
        let values = self.values.as_ref().ok_or("no value levels")?;
        let colours = self.colours.as_ref().ok_or("no colour levels")?;
        let edges = self.contours.as_ref().ok_or("no contours")?;
        let (iw, ih) = img.work.dimensions();
        let water = pal.medium.uses_water();
        let k = self.max_paints;
        let date = today();

        let mut p = Pages {
            doc: Doc::new(A4_W, A4_H),
            y: MARGIN,
            footer: format!("PaletteCraft  -  reference brochure for \"{}\"  -  {}", img.name, date),
        };
        let cw = p.content_w();

        let original = p.doc.add_image(&img.work);
        let greys: Vec<[u8; 3]> = values
            .lightness
            .iter()
            .map(|&l| {
                let g = crate::colour::grey_for_lightness(l);
                [g, g, g]
            })
            .collect();
        let value_img = p.doc.add_image(&rgba_to_rgb(&imgproc::render_levels(&values.index, &greys, None), iw, ih));
        let contour_img = p.doc.add_image(&rgba_to_rgb(&imgproc::render_contours(edges), iw, ih));
        let colour_img = p.doc.add_image(&rgba_to_rgb(&imgproc::render_levels(&colours.index, &colours.srgb, None), iw, ih));

        // ---- page 1: cover with the original picture ----
        p.page();
        p.doc.text(MARGIN, p.y + 10.0, 10.0, true, ACCENT, "REFERENCE BROCHURE");
        p.y += 36.0;
        let title = fit(&img.name, 24.0, true, cw);
        p.doc.text(MARGIN, p.y, 24.0, true, INK, &title);
        p.y += 20.0;
        let mixes = if k == 1 { "single paints".to_string() } else { format!("mixes of up to {k} paints") };
        let sub = format!("{}  -  palette \"{}\"  -  {}  -  {}", pal.medium.label(), pal.name, mixes, date);
        p.doc.text(MARGIN, p.y, 10.5, false, MUTED, &fit(&sub, 10.5, false, cw));
        p.y += 14.0;
        p.doc.line(MARGIN, p.y, MARGIN + cw, p.y, RULE, 0.8);
        p.y += 18.0;
        p.picture(original, iw, ih, cw, 440.0, false);
        p.y += 24.0;
        let (size_note, crop_note) = match img.crop {
            Some((_, _, w, h)) => (format!("{w} x {h} px"), format!(" (cropped from {} x {} px)", img.original.width(), img.original.height())),
            None => (format!("{} x {} px", img.original.width(), img.original.height()), String::new()),
        };
        let lighten = if !self.allow_lighten {
            "not used".to_string()
        } else if water {
            "with water over white paper".to_string()
        } else {
            format!("with white paint ({})", mixer.white_name)
        };
        let rows: Vec<(&str, String)> = vec![
            ("Picture", format!("{size_note}{crop_note}")),
            ("Paint", pal.medium.label().to_string()),
            ("Palette", format!("{} ({} paints)", pal.name, pal.paints.len())),
            ("Value levels", values.lightness.len().to_string()),
            ("Colour levels", colours.srgb.len().to_string()),
            ("Paints per mix", format!("up to {k}")),
            ("Lighter colours", lighten),
            ("Mixing model", self.mix_model.describe().to_string()),
            ("Contour detail", format!("{} of {}", self.contour_detail, imgproc::CONTOUR_DETAIL_MAX)),
        ];
        for (key, val) in rows {
            p.ensure(16.0);
            p.doc.text(MARGIN, p.y, 10.0, false, MUTED, key);
            p.doc.text(MARGIN + 110.0, p.y, 10.0, false, INK, &fit(&val, 10.0, false, cw - 110.0));
            p.y += 16.0;
        }
        p.y += 8.0;
        p.ensure(20.0);
        p.doc.text(MARGIN, p.y, 9.0, false, MUTED, "Inside: value study and contours  -  palette and mixing chart  -  colour mixes for every colour level");

        // ---- page 2: value study and contours ----
        p.page();
        let n = values.lightness.len();
        p.heading(
            "Value study",
            &format!(
                "The picture in {n} values, from darkest (1) to lightest ({n}). The percentage under each value is the share of the picture's area that has that value."
            ),
        );
        let scale_h = 34.0;
        // two pictures share what is left of the page after the scale and the second heading
        let avail = p.bottom() - p.y - (scale_h + 26.0) - 92.0;
        let pic_h = (avail / 2.0).max(120.0);
        p.picture(value_img, iw, ih, cw, pic_h, true);
        p.y += 10.0;
        let bw = (cw / n as f32).min(78.0);
        let sx = MARGIN + (cw - bw * n as f32) / 2.0;
        for i in 0..n {
            let x = sx + i as f32 * bw;
            p.doc.rect(x, p.y, bw - 3.0, scale_h, greys[i]);
            p.doc.rect_stroke(x, p.y, bw - 3.0, scale_h, RULE, 0.5);
            let tc = if values.lightness[i] > 55.0 { INK } else { [250, 250, 250] };
            p.doc.text(x + 5.0, p.y + 13.0, 10.0, true, tc, &(i + 1).to_string());
            p.doc.text(x + 5.0, p.y + 27.0, 9.0, false, tc, &format!("{:.0}%", values.share[i] * 100.0));
        }
        p.y += scale_h + 26.0;
        p.heading(
            "Contours",
            &format!("Line drawing of the picture's edges (detail {} of {}), for transferring the drawing to paper or canvas.", self.contour_detail, imgproc::CONTOUR_DETAIL_MAX),
        );
        // same size as the value picture, so the two can be compared
        let left = (p.bottom() - p.y).max(120.0);
        p.picture(contour_img, iw, ih, cw, pic_h.min(left), true);

        // ---- page 3: palette ----
        p.page();
        let medium_note = if water { "lightened with water over paper" } else { "lightened with white paint" };
        p.heading(
            &format!("Palette: {}", pal.name),
            &format!(
                "{} paints, {}. The sign in each dot shows transparency: ring = transparent, half = half-transparent, solid = opaque. Colours are the masstone (full strength).",
                pal.medium.label(),
                medium_note
            ),
        );
        let cols = [MARGIN + 30.0, MARGIN + 220.0, MARGIN + 320.0, MARGIN + 440.0];
        p.doc.text(cols[0], p.y + 4.0, 8.5, true, MUTED, "PAINT");
        p.doc.text(cols[1], p.y + 4.0, 8.5, true, MUTED, "PIGMENT");
        p.doc.text(cols[2], p.y + 4.0, 8.5, true, MUTED, "TRANSPARENCY");
        p.doc.text(cols[3], p.y + 4.0, 8.5, true, MUTED, "STRENGTH");
        p.y += 12.0;
        for paint in &pal.paints {
            p.ensure(24.0);
            paint_dot(&mut p.doc, MARGIN + 10.0, p.y + 10.0, 9.0, paint.color, paint.transparency);
            p.doc.text(cols[0], p.y + 14.0, 10.5, true, INK, &fit(&paint.name, 10.5, true, 190.0));
            p.doc.text(cols[1], p.y + 14.0, 10.0, false, ACCENT, &fit(&paint.pigment, 10.0, false, 95.0));
            p.doc.text(cols[2], p.y + 14.0, 10.0, false, INK, paint.transparency.label());
            p.doc.text(cols[3], p.y + 14.0, 10.0, false, INK, &format!("{:.1}", paint.strength));
            p.y += 22.0;
            p.doc.line(MARGIN, p.y, MARGIN + cw, p.y, RULE, 0.4);
        }
        let np = pal.paints.len();
        if np > 0 {
            let steps: Vec<Lighten> = if water {
                [1.0, 0.55, 0.3, 0.16, 0.075].iter().map(|&l| Lighten::Water(l)).collect()
            } else {
                [0u32, 1, 3, 8, 24].iter().map(|&n| if n == 0 { Lighten::None } else { Lighten::White(n) }).collect()
            };
            let cell = ((cw - 160.0) / (np + 1) as f32).min(30.0);
            let chart_h = cell * (np + 1) as f32 + 30.0;
            p.y += 18.0;
            p.ensure(chart_h + 50.0);
            p.small_heading("Mixing chart");
            p.y += 4.0;
            let note = if water {
                "Each square: equal parts of the row and column paint. Right: each paint with more and more water."
            } else {
                "Each square: equal parts of the row and column paint. Right: each paint with 1, 3, 8 and 24 parts of white."
            };
            p.doc.text(MARGIN, p.y, 9.0, false, MUTED, note);
            p.y += 10.0;
            let (ox, oy) = (MARGIN, p.y);
            for i in 0..np {
                let c = pal.paints[i].color;
                p.doc.circle(ox + cell * (i as f32 + 1.5), oy + cell * 0.5, cell * 0.34, c);
                p.doc.circle(ox + cell * 0.5, oy + cell * (i as f32 + 1.5), cell * 0.34, c);
                for j in 0..np {
                    let mix = if i == j { mixer.mix_srgb(&[(i, 1)], Lighten::None) } else { mixer.mix_srgb(&[(i, 1), (j, 1)], Lighten::None) };
                    p.doc.rect(ox + cell * (j as f32 + 1.0) + 0.6, oy + cell * (i as f32 + 1.0) + 0.6, cell - 1.2, cell - 1.2, mix);
                }
            }
            let lx = ox + cell * (np as f32 + 1.0) + 24.0;
            let tc = ((MARGIN + cw - lx) / steps.len() as f32).min(cell.max(22.0));
            p.doc.text(lx, oy + cell * 0.65, 8.5, false, MUTED, if water { "with water ->" } else { "with white ->" });
            for i in 0..np {
                for (s, l) in steps.iter().enumerate() {
                    let c = mixer.mix_srgb(&[(i, 1)], *l);
                    p.doc.rect(lx + s as f32 * tc + 0.6, oy + cell * (i as f32 + 1.0) + 0.6, tc - 1.2, cell - 1.2, c);
                }
            }
            p.y = oy + cell * (np as f32 + 1.0) + 8.0;
        }

        // ---- page 4+: colour mixes ----
        p.page();
        let nc = colours.srgb.len();
        let up_to = if k == 1 { "single paints".to_string() } else { format!("up to {k} paints") };
        let light = if !self.allow_lighten {
            String::new()
        } else if water {
            " Water is given as consistency and pigment strength.".to_string()
        } else {
            format!(" White is {}.", mixer.white_name)
        };
        p.heading(
            "Colour mixes",
            &format!(
                "Recipes for the {nc} colour levels of the picture, using {up_to} from \"{}\". Parts are by volume.{light} The mixes are Kubelka-Munk predictions from estimated paint data, so test them on a scrap first. The match rating (dE) is explained after the list.",
                pal.name
            ),
        );
        p.picture(colour_img, iw, ih, cw, 180.0, true);
        p.y += 16.0;
        let nv = values.lightness.len();
        let row_h = 42.0;
        let row = |p: &mut Pages, label: &str, target: [u8; 3], recipe: Option<&Recipe>, info: &str| {
            p.ensure(row_h);
            let y = p.y;
            p.doc.text(MARGIN, y + 21.0, 10.0, true, MUTED, label);
            p.doc.rect(MARGIN + 24.0, y + 4.0, 30.0, 34.0, target);
            p.doc.rect_stroke(MARGIN + 24.0, y + 4.0, 30.0, 34.0, RULE, 0.5);
            let tx = MARGIN + 104.0;
            match recipe {
                Some(rc) => {
                    p.doc.rect(MARGIN + 56.0, y + 4.0, 30.0, 34.0, rc.srgb);
                    p.doc.rect_stroke(MARGIN + 56.0, y + 4.0, 30.0, 34.0, RULE, 0.5);
                    let parts = mixer.describe_parts(rc);
                    p.doc.text(tx, y + 16.0, 10.5, true, INK, &fit(&parts, 10.5, true, cw - 104.0 - 120.0));
                    p.doc.text(tx, y + 30.0, 9.0, false, MUTED, &fit(&mixer.describe_lighten(rc), 9.0, false, cw - 104.0 - 120.0));
                    let m = format!("{}  (dE {:.1})", match_label(rc.de), rc.de);
                    p.doc.text_right(MARGIN + cw, y + 30.0, 9.0, true, match_colour(rc.de), &m);
                }
                None => p.doc.text(tx, y + 22.0, 10.0, false, MUTED, "no paints to mix with"),
            }
            p.doc.text_right(MARGIN + cw, y + 16.0, 8.5, false, MUTED, info);
            p.y += row_h;
            p.doc.line(MARGIN, p.y - 2.0, MARGIN + cw, p.y - 2.0, RULE, 0.4);
        };
        p.doc.text(MARGIN + 24.0, p.y, 7.5, true, MUTED, "PICTURE");
        p.doc.text(MARGIN + 62.0, p.y, 7.5, true, MUTED, "MIX");
        p.doc.text(MARGIN + 104.0, p.y, 7.5, true, MUTED, "RECIPE");
        p.y += 4.0;
        for i in 0..nc {
            let v = imgproc::value_level_of(colours.lab[i][0], &img.analysis, nv) + 1;
            let info = format!("{:.0}% of the picture  -  value {v}/{nv}", colours.share[i] * 100.0);
            row(&mut p, &format!("{}", i + 1), colours.srgb[i], self.recipes.get(i).and_then(|r| r.as_ref()), &info);
        }
        if let Some(pk) = &self.picked {
            p.y += 10.0;
            p.small_heading("Clicked spot");
            row(&mut p, "*", pk.srgb, pk.recipe.as_ref(), "the spot you clicked on the picture");
        }
        match_legend(&mut p);
        let pages = p.doc.page_count();
        Ok((p.doc.finish(), pages))
    }
}
