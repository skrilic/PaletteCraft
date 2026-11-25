//! Painting screen: picture controls on the left, canvas in the middle, paints and recipes on the right.

use super::*;
use crate::grid::{LINE_COLOURS, draw_overlay};
use crate::ui::{ACCENT, ACCENT_DIM, PANEL2, rrect, rrect_lines, swatch};

impl App {
    pub(super) fn painting(&mut self, ui: &mut Ui) {
        if Self::compact() {
            self.painting_compact(ui);
            return;
        }
        let (w, h) = (screen_width(), screen_height());
        let canvas = Rect::new(LEFT_W, TOP_BAR, (w - LEFT_W - RIGHT_W).max(100.0), h - TOP_BAR - BOTTOM_STRIP);
        self.canvas(ui, canvas);
        self.bottom_strip(ui, Rect::new(canvas.x, canvas.y + canvas.h, canvas.w, BOTTOM_STRIP));
        self.left_panel(ui, Rect::new(0.0, TOP_BAR, LEFT_W, h - TOP_BAR));
        self.right_panel(ui, Rect::new(w - RIGHT_W, TOP_BAR, RIGHT_W, h - TOP_BAR));
    }

    /// Compact layout: one pane at a time, picked in the pane bar.
    fn painting_compact(&mut self, ui: &mut Ui) {
        let (w, h) = (screen_width(), screen_height());
        let top = Self::content_top();
        let area = Rect::new(0.0, top, w, h - top);
        if self.cropping {
            // the crop frame is dragged on the picture
            self.pane_painting = 0;
        }
        match self.pane_painting {
            0 => self.picture_pane(ui, area),
            1 => {
                let r = Rect::new(0.0, area.y - self.pane_scroll, w, area.h + self.pane_scroll);
                let end = self.left_panel(ui, r);
                let mut scroll = self.pane_scroll;
                ui.scroll_px(area, &mut scroll, end + 24.0 - r.y - area.h);
                self.pane_scroll = scroll;
            }
            _ => self.right_panel(ui, area),
        }
    }

    /// Compact layout, "Picture" pane: a row of picture buttons, the picture, the value or
    /// colour strip and the recipe of the clicked spot.
    fn picture_pane(&mut self, ui: &mut Ui, area: Rect) {
        let x = area.x + 10.0;
        let bw = area.w - 20.0;
        let y = area.y + 8.0;
        let row_h = 32.0;
        draw_rectangle(area.x, area.y, area.w, row_h + 16.0, PANEL);
        if self.cropping {
            let names: Vec<String> = CROP_ASPECTS.iter().map(|a| format!("Shape: {}", a.0)).collect();
            let dw = (bw * 0.42).round();
            if let Some(i) = ui.dropdown("crop_aspect", Rect::new(x, y, dw, row_h), &names[self.crop_aspect], &names) {
                self.crop_aspect = i;
                self.fit_crop_aspect();
            }
            let half = (bw - dw - 12.0) / 2.0;
            if ui.button("crop_apply", Rect::new(x + dw + 6.0, y, half, row_h), "Apply crop") {
                self.apply_crop();
            }
            if ui.button("crop_cancel", Rect::new(x + dw + half + 12.0, y, half, row_h), "Cancel") {
                self.cropping = false;
            }
        } else {
            let ow = (bw * 0.42).round();
            if ui.picture_button("open_image", Rect::new(x, y, ow, row_h), "Open picture...") {
                self.open_dialog();
            }
            let menu = Rect::new(x + ow + 4.0, y, 32.0, row_h);
            if let Some(i) = ui.menu_button("open_recent", menu, Rect::new(x, y, ow + 36.0, row_h), || self.recent_menu_items()) {
                self.recent_menu_pick(ui, i);
            }
            let rest = bw - ow - 42.0;
            let sx = menu.x + menu.w + 6.0;
            if ui.button("sample_image", Rect::new(sx, y, rest * 0.55 - 3.0, row_h), "Sample") {
                self.set_image("Sample landscape".into(), imgproc::sample_landscape(1200, 800));
            }
            let cr = Rect::new(sx + rest * 0.55 + 3.0, y, rest * 0.45 - 3.0, row_h);
            if ui.button("crop_start", cr, "Crop...") && self.img.is_some() {
                self.start_crop();
            }
        }
        // a phone held sideways has no height to spare: the recipe is under "Recipes" then
        let card_h = if self.cropping || area.h < 480.0 { 0.0 } else { PICKED_CARD_H };
        let canvas_y = area.y + row_h + 16.0;
        let canvas = Rect::new(area.x, canvas_y, area.w, (area.y + area.h - canvas_y - BOTTOM_STRIP - card_h).max(80.0));
        self.canvas(ui, canvas);
        self.bottom_strip(ui, Rect::new(area.x, canvas.y + canvas.h, area.w, BOTTOM_STRIP));
        if card_h > 0.0 {
            let cy = canvas.y + canvas.h + BOTTOM_STRIP;
            draw_rectangle(area.x, cy, area.w, area.y + area.h - cy, PANEL);
            draw_line(area.x, cy, area.x + area.w, cy, 1.0, BORDER);
            self.picked_section(ui, x + 6.0, cy + 22.0, bw - 12.0);
        }
    }

    /// Panel of picture controls; returns the y where its content ends.
    fn left_panel(&mut self, ui: &mut Ui, r: Rect) -> f32 {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_line(r.x + r.w, r.y, r.x + r.w, r.y + r.h, 1.0, BORDER);
        let x = r.x + 16.0;
        let bw = r.w - 32.0;
        let mut y = r.y + 22.0;
        ui.header("PICTURE", x, y);
        y += 10.0;
        let open = Rect::new(x, y, bw - 38.0, 32.0);
        if ui.picture_button("open_image", open, "Open picture...") {
            self.open_dialog();
        }
        let menu = Rect::new(x + bw - 32.0, y, 32.0, 32.0);
        if let Some(i) = ui.menu_button("open_recent", menu, Rect::new(x, y, bw, 32.0), || self.recent_menu_items()) {
            self.recent_menu_pick(ui, i);
        }
        ui.tooltip("open_recent_tip", menu, || vec![("Recently opened pictures".to_string(), TEXT)]);
        y += 38.0;
        if ui.button("sample_image", Rect::new(x, y, bw, 28.0), "Use sample picture") {
            self.set_image("Sample landscape".into(), imgproc::sample_landscape(1200, 800));
        }
        y += 46.0;
        if let Some(img) = &self.img {
            ui.text_fit(&img.name, x, y, 14.0, TEXT, bw);
            let (ww, hh) = match img.crop {
                Some((_, _, cw, ch)) => (cw, ch),
                None => img.original.dimensions(),
            };
            let note = if img.crop.is_some() { " (cropped)" } else { "" };
            ui.text(&format!("{ww} x {hh} px{note}"), x, y + 18.0, 13.0, MUTED);
        } else {
            ui.text("No picture yet", x, y, 14.0, MUTED);
        }
        y += 44.0;

        ui.header("CROP", x, y);
        y += 10.0;
        let half = bw / 2.0 - 3.0;
        if self.cropping {
            let names: Vec<String> = CROP_ASPECTS.iter().map(|a| format!("Shape: {}", a.0)).collect();
            if let Some(i) = ui.dropdown("crop_aspect", Rect::new(x, y, bw, 30.0), &names[self.crop_aspect], &names) {
                self.crop_aspect = i;
                self.fit_crop_aspect();
            }
            y += 36.0;
            if ui.button("crop_apply", Rect::new(x, y, half, 30.0), "Apply crop") {
                self.apply_crop();
            }
            if ui.button("crop_cancel", Rect::new(x + half + 6.0, y, half, 30.0), "Cancel") {
                self.cropping = false;
            }
            y += 36.0;
            ui.text("Drag on the picture to frame it", x, y + 6.0, 13.0, MUTED);
            y += 16.0;
        } else {
            if ui.button("crop_start", Rect::new(x, y, half, 30.0), "Crop...") && self.img.is_some() {
                self.start_crop();
            }
            let cropped = self.img.as_ref().is_some_and(|i| i.crop.is_some());
            if ui.button("crop_reset", Rect::new(x + half + 6.0, y, half, 30.0), "Undo crop") && cropped {
                self.reset_crop();
            }
            y += 36.0;
        }
        y += 20.0;

        ui.header("VIEW", x, y);
        y += 10.0;
        for (i, v) in View::ALL.iter().enumerate() {
            let br = Rect::new(x + (i % 2) as f32 * (half + 6.0), y + (i / 2) as f32 * 32.0, half, 28.0);
            if ui.button_ex(v.id(), br, v.label(), self.view == *v) {
                self.view = *v;
                self.tex_dirty = true;
            }
        }
        // sixth cell: contour lines over the other views
        let cr = Rect::new(x + half + 12.0, y + 2.0 * 32.0, half - 6.0, 28.0);
        if ui.checkbox("contours_on_top", cr, "Lines on top", &mut self.contours_on_top) {
            self.tex_dirty = true;
        }
        y += 3.0 * 32.0;
        if self.view == View::Contours || self.contours_on_top {
            y += 14.0;
            ui.text("Contour detail", x, y + 4.0, 15.0, TEXT);
            let s = self.contour_detail.to_string();
            ui.text(&s, x + bw - ui.measure(&s, 16.0), y + 4.0, 16.0, ACCENT);
            ui.hud("contour_detail_label");
            y += 10.0;
            let (lo, hi) = (imgproc::CONTOUR_DETAIL_MIN, imgproc::CONTOUR_DETAIL_MAX);
            if ui.slider_int("contour_detail", Rect::new(x - 6.0, y, bw + 12.0, 24.0), &mut self.contour_detail, lo, hi) {
                self.contours = None;
                self.tex_dirty = true;
            }
            y += 30.0;
        }
        y += 10.0;
        ui.text("Value levels", x, y + 4.0, 15.0, TEXT);
        let s = self.value_levels.to_string();
        ui.text(&s, x + bw - ui.measure(&s, 16.0), y + 4.0, 16.0, ACCENT);
        ui.hud("value_levels_label");
        y += 10.0;
        if ui.slider_int("value_levels", Rect::new(x - 6.0, y, bw + 12.0, 24.0), &mut self.value_levels, 2, 10) {
            self.values = None;
            if self.view == View::Colours || self.view == View::Original {
                self.view = View::Values;
            }
            self.tex_dirty = true;
        }
        y += 36.0;
        ui.text("Colour levels", x, y + 4.0, 15.0, TEXT);
        let s = self.colour_levels.to_string();
        ui.text(&s, x + bw - ui.measure(&s, 16.0), y + 4.0, 16.0, ACCENT);
        ui.hud("colour_levels_label");
        y += 10.0;
        if ui.slider_int("colour_levels", Rect::new(x - 6.0, y, bw + 12.0, 24.0), &mut self.colour_levels, 2, 24) {
            self.colours = None;
            self.highlight = None;
            self.recipe_scroll = 0;
            self.view = View::Colours;
            self.tex_dirty = true;
        }
        y += 46.0;

        ui.header("OVERLAY", x, y);
        y += 10.0;
        let ids = ["overlay_none", "overlay_thirds", "overlay_golden", "overlay_fib", "overlay_grid"];
        let short = ["None", "Rule of thirds", "Golden ratio", "Fibonacci", "Grid"];
        for (i, k) in Overlay::ALL.iter().enumerate() {
            let br = Rect::new(x + (i % 2) as f32 * (half + 6.0), y + (i / 2) as f32 * 32.0, half, 28.0);
            if ui.button_ex(ids[i], br, short[i], self.overlay.kind == *k) {
                self.overlay.kind = *k;
            }
            if *k == Overlay::Golden {
                ui.tooltip("overlay_golden_tip", br, || {
                    vec![
                        ("Golden ratio grid (phi grid)".to_string(), TEXT),
                        ("Lines at 38.2% and 61.8% of the width and height:".to_string(), MUTED),
                        ("a tighter, calmer placement than the rule of thirds.".to_string(), MUTED),
                    ]
                });
            }
        }
        y += 102.0;
        match self.overlay.kind {
            Overlay::Grid => {
                ui.text("Cells on the long side", x, y + 4.0, 14.0, TEXT);
                let s = self.overlay.cells.to_string();
                ui.text(&s, x + bw - ui.measure(&s, 15.0), y + 4.0, 15.0, ACCENT);
                y += 8.0;
                ui.slider_int("grid_cells", Rect::new(x - 6.0, y, bw + 12.0, 22.0), &mut self.overlay.cells, 2, 20);
                y += 28.0;
                ui.checkbox("grid_diagonals", Rect::new(x, y, bw, 22.0), "Diagonals", &mut self.overlay.diagonals);
                y += 30.0;
            }
            Overlay::Fibonacci => {
                if ui.button_ex("fib_flip_x", Rect::new(x, y, half, 26.0), "Mirror sideways", self.overlay.flip_x) {
                    self.overlay.flip_x = !self.overlay.flip_x;
                }
                if ui.button_ex("fib_flip_y", Rect::new(x + half + 6.0, y, half, 26.0), "Mirror up/down", self.overlay.flip_y) {
                    self.overlay.flip_y = !self.overlay.flip_y;
                }
                y += 34.0;
            }
            _ => {}
        }
        if self.overlay.kind != Overlay::None {
            let names: Vec<&str> = LINE_COLOURS.iter().map(|c| c.0).collect();
            if let Some(i) = ui.segmented("line_colour", Rect::new(x, y, bw + 3.0, 26.0), &names, self.overlay.colour) {
                self.overlay.colour = i;
            }
            y += 32.0;
            ui.text("Line strength", x, y + 14.0, 13.0, MUTED);
            ui.slider_f("line_opacity", Rect::new(x + 90.0, y, bw - 84.0, 22.0), &mut self.overlay.opacity, 0.15, 1.0);
            y += 26.0;
        }
        let hy = r.y + r.h - 40.0;
        if hy > y + 24.0 && !Self::compact() {
            ui.text("Tip: drop a picture file on the", x, hy, 13.0, MUTED);
            ui.text("window to open it.", x, hy + 17.0, 13.0, MUTED);
        }
        y
    }

    fn fit_crop_aspect(&mut self) {
        let (ratio, Some(img), Some(sel)) = (CROP_ASPECTS[self.crop_aspect].1, &self.img, self.crop_sel) else { return };
        if ratio <= 0.0 {
            return;
        }
        let (ow, oh) = (img.original.width() as f32, img.original.height() as f32);
        // keep the centre, fit the largest frame of that shape inside the current one
        let (cx, cy) = (sel.x + sel.w / 2.0, sel.y + sel.h / 2.0);
        let mut wn = sel.w;
        let mut hn = wn * ow / (ratio * oh);
        if hn > sel.h {
            hn = sel.h;
            wn = hn * ratio * oh / ow;
        }
        let x = (cx - wn / 2.0).clamp(0.0, 1.0 - wn);
        let y = (cy - hn / 2.0).clamp(0.0, 1.0 - hn);
        self.crop_sel = Some(Rect::new(x, y, wn, hn));
    }

    fn fit_rect(area: Rect, iw: f32, ih: f32) -> Rect {
        let pad = if Self::compact() { 8.0 } else { 18.0 };
        let aw = (area.w - 2.0 * pad).max(10.0);
        let ah = (area.h - 2.0 * pad).max(10.0);
        let s = (aw / iw).min(ah / ih);
        let (w, h) = (iw * s, ih * s);
        Rect::new((area.x + (area.w - w) / 2.0).round(), (area.y + (area.h - h) / 2.0).round(), w.round(), h.round())
    }

    fn canvas(&mut self, ui: &mut Ui, area: Rect) {
        draw_rectangle(area.x, area.y, area.w, area.h, Color::new(0.08, 0.085, 0.095, 1.0));
        ui.mark("canvas", area);
        let Some(img) = &self.img else {
            let hint = if Self::compact() { "Open a picture or use the sample" } else { "Open a picture or use the sample to start" };
            ui.text_centered(hint, area, 18.0, MUTED);
            return;
        };
        if self.cropping {
            let (ow, oh) = (img.original.width() as f32, img.original.height() as f32);
            let ir = Self::fit_rect(area, ow, oh);
            self.canvas_rect = ir;
            ui.mark("canvas_image", ir);
            if let Some(t) = &self.crop_tex {
                draw_texture_ex(t, ir.x, ir.y, WHITE, DrawTextureParams { dest_size: Some(vec2(ir.w, ir.h)), ..Default::default() });
            }
            self.crop_interact(ui, ir, ow, oh);
            if let Some(s) = self.crop_sel {
                let sr = Rect::new(ir.x + s.x * ir.w, ir.y + s.y * ir.h, s.w * ir.w, s.h * ir.h);
                let shade = Color::new(0.0, 0.0, 0.0, 0.6);
                draw_rectangle(ir.x, ir.y, ir.w, sr.y - ir.y, shade);
                draw_rectangle(ir.x, sr.y + sr.h, ir.w, ir.y + ir.h - sr.y - sr.h, shade);
                draw_rectangle(ir.x, sr.y, sr.x - ir.x, sr.h, shade);
                draw_rectangle(sr.x + sr.w, sr.y, ir.x + ir.w - sr.x - sr.w, sr.h, shade);
                for i in 1..3 {
                    let fx = sr.x + sr.w * i as f32 / 3.0;
                    let fy = sr.y + sr.h * i as f32 / 3.0;
                    draw_line(fx, sr.y, fx, sr.y + sr.h, 1.0, Color::new(1.0, 1.0, 1.0, 0.35));
                    draw_line(sr.x, fy, sr.x + sr.w, fy, 1.0, Color::new(1.0, 1.0, 1.0, 0.35));
                }
                draw_rectangle_lines(sr.x, sr.y, sr.w, sr.h, 2.0, ACCENT);
                for c in corners(sr) {
                    draw_rectangle(c.x - 6.0, c.y - 6.0, 12.0, 12.0, ACCENT);
                }
                let label = format!("{} x {} px", (s.w * ow).round(), (s.h * oh).round());
                let lw = ui.measure(&label, 14.0);
                rrect(Rect::new(sr.x + 8.0, sr.y + 8.0, lw + 14.0, 22.0), 4.0, Color::new(0.0, 0.0, 0.0, 0.7));
                ui.text(&label, sr.x + 15.0, sr.y + 24.0, 14.0, TEXT);
            }
            return;
        }
        let (iw, ih) = (img.work.width() as f32, img.work.height() as f32);
        let ir = Self::fit_rect(area, iw, ih);
        self.canvas_rect = ir;
        ui.mark("canvas_image", ir);
        if let Some(t) = &self.texture {
            draw_texture_ex(t, ir.x, ir.y, WHITE, DrawTextureParams { dest_size: Some(vec2(ir.w, ir.h)), ..Default::default() });
        }
        let font = ui.font.clone();
        let label = move |s: &str, x: f32, y: f32, c: Color| {
            draw_text_ex(s, x, y, TextParams { font: font.as_ref(), font_size: 12, color: c, ..Default::default() });
        };
        self.overlay_segments = draw_overlay(ir, &self.overlay, &label);
        if self.overlay_segments > 0 {
            ui.hud("overlay");
        }
        if let Some(p) = &self.picked {
            let c = vec2(ir.x + p.pos.x * ir.w, ir.y + p.pos.y * ir.h);
            draw_circle_lines(c.x, c.y, 9.0, 2.5, BLACK);
            draw_circle_lines(c.x, c.y, 7.0, 2.0, WHITE);
        }
        if ui.clicked(ir) {
            let n = (ui.input.mouse - ir.point()) / ir.size();
            let px = ((n.x * iw) as u32).min(iw as u32 - 1);
            let py = ((n.y * ih) as u32).min(ih as u32 - 1);
            if self.view == View::Colours {
                if let Some(c) = &self.colours {
                    let lvl = c.index[(py * iw as u32 + px) as usize] as usize;
                    self.highlight = if self.highlight == Some(lvl) { None } else { Some(lvl) };
                    self.recipe_scroll = lvl.saturating_sub(1);
                    self.tex_dirty = true;
                }
            } else {
                self.pick_at(n);
            }
        }
    }

    fn crop_interact(&mut self, ui: &mut Ui, ir: Rect, ow: f32, oh: f32) {
        let m = ui.input.mouse;
        let n = ((m - ir.point()) / ir.size()).clamp(Vec2::ZERO, Vec2::ONE);
        let grab = Rect::new(ir.x - 10.0, ir.y - 10.0, ir.w + 20.0, ir.h + 20.0);
        if ui.pressed_in(grab) {
            let mut mode = CropDrag::New(n);
            if let Some(s) = self.crop_sel {
                let sr = Rect::new(ir.x + s.x * ir.w, ir.y + s.y * ir.h, s.w * ir.w, s.h * ir.h);
                if let Some(ci) = corners(sr).iter().position(|c| c.distance(m) < 12.0) {
                    mode = CropDrag::Corner(ci);
                } else if sr.contains(m) {
                    mode = CropDrag::Move(n - vec2(s.x, s.y));
                }
            }
            self.crop_drag = Some(mode);
        }
        if !ui.input.down && !ui.input.pressed {
            self.crop_drag = None;
        }
        let Some(drag) = self.crop_drag else { return };
        let ratio = CROP_ASPECTS[self.crop_aspect].1;
        let shape = |a: Vec2, b: Vec2| -> Rect {
            let mut w = (b.x - a.x).abs();
            let mut h = (b.y - a.y).abs();
            if ratio > 0.0 {
                let h_for_w = w * ow / (ratio * oh);
                if h_for_w <= h || h < 1e-4 {
                    h = h_for_w;
                } else {
                    w = h * ratio * oh / ow;
                }
            }
            let x = if b.x < a.x { a.x - w } else { a.x };
            let y = if b.y < a.y { a.y - h } else { a.y };
            let x0 = x.max(0.0);
            let y0 = y.max(0.0);
            Rect::new(x0, y0, (x + w).min(1.0) - x0, (y + h).min(1.0) - y0)
        };
        match drag {
            CropDrag::New(a) => self.crop_sel = Some(shape(a, n)),
            CropDrag::Move(off) => {
                if let Some(s) = self.crop_sel {
                    let x = (n.x - off.x).clamp(0.0, 1.0 - s.w);
                    let y = (n.y - off.y).clamp(0.0, 1.0 - s.h);
                    self.crop_sel = Some(Rect::new(x, y, s.w, s.h));
                }
            }
            CropDrag::Corner(i) => {
                if let Some(s) = self.crop_sel {
                    // the opposite corner stays put
                    let cs = [vec2(s.x, s.y), vec2(s.x + s.w, s.y), vec2(s.x + s.w, s.y + s.h), vec2(s.x, s.y + s.h)];
                    self.crop_sel = Some(shape(cs[(i + 2) % 4], n));
                }
            }
        }
    }

    fn bottom_strip(&mut self, ui: &mut Ui, r: Rect) {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_line(r.x, r.y, r.x + r.w, r.y, 1.0, BORDER);
        if self.img.is_none() || self.cropping {
            return;
        }
        let x = r.x + 16.0;
        let bx = x + 62.0;
        match self.view {
            View::Original | View::Greyscale | View::Values | View::Contours => {
                let Some(v) = &self.values else { return };
                ui.text("Values", x, r.y + 34.0, 14.0, MUTED);
                let n = v.lightness.len();
                let bw = ((r.w - 90.0) / n as f32).min(80.0);
                let mut tip = None;
                for i in 0..n {
                    let g = crate::colour::grey_for_lightness(v.lightness[i]);
                    let br = Rect::new(bx + i as f32 * bw, r.y + 10.0, bw - 4.0, 38.0);
                    swatch(br, [g, g, g]);
                    let tc = if v.lightness[i] > 55.0 { BLACK } else { WHITE };
                    ui.text(&format!("{}", i + 1), br.x + 6.0, br.y + 16.0, 14.0, tc);
                    ui.text(&format!("{:.0}%", v.share[i] * 100.0), br.x + 6.0, br.y + 32.0, 12.0, tc);
                    ui.mark(&format!("value_box_{i}"), br);
                    if ui.hovered(br) {
                        let where_ = match i {
                            0 => " (darkest)".to_string(),
                            _ if i + 1 == n => " (lightest)".to_string(),
                            _ => String::new(),
                        };
                        let text = format!(
                            "Value {} of {}{}: {:.0}% of the picture's area has this value",
                            i + 1,
                            n,
                            where_,
                            v.share[i] * 100.0
                        );
                        tip = Some((text, br));
                    }
                }
                ui.mark("value_scale", Rect::new(bx, r.y + 10.0, bw * n as f32, 38.0));
                ui.hud("value_scale");
                // Caption explaining the numbers, when there is room beside the boxes.
                let cx = bx + bw * n as f32 + 10.0;
                if r.x + r.w - cx > 170.0 {
                    ui.text("1 = darkest value", cx, r.y + 24.0, 12.0, MUTED);
                    ui.text("% = share of the picture's area", cx, r.y + 40.0, 12.0, MUTED);
                    ui.hud("value_scale_caption");
                }
                if let Some((text, br)) = tip {
                    let tw = ui.measure(&text, 14.0);
                    let tx = (br.x + br.w / 2.0 - tw / 2.0).clamp(r.x + 4.0, r.x + r.w - tw - 14.0);
                    let tr = Rect::new(tx - 6.0, br.y - 34.0, tw + 12.0, 24.0);
                    rrect(tr, 4.0, BG);
                    rrect_lines(tr, 4.0, BORDER);
                    ui.text(&text, tx, tr.y + 17.0, 14.0, TEXT);
                    ui.hud("value_tip");
                }
            }
            View::Colours => {
                let Some(c) = &self.colours else { return };
                ui.text("Colours", x, r.y + 34.0, 14.0, MUTED);
                let n = c.srgb.len();
                let bw = ((r.w - 90.0) / n as f32).min(64.0);
                let mut clicked = None;
                for i in 0..n {
                    let br = Rect::new(bx + i as f32 * bw, r.y + 10.0, bw - 4.0, 38.0);
                    swatch(br, c.srgb[i]);
                    if self.highlight == Some(i) {
                        rrect_lines(Rect::new(br.x - 2.0, br.y - 2.0, br.w + 4.0, br.h + 4.0), 5.0, ACCENT);
                    }
                    let tc = if c.lab[i][0] > 55.0 { BLACK } else { WHITE };
                    if bw > 22.0 {
                        ui.text(&format!("{}", i + 1), br.x + 5.0, br.y + 16.0, 13.0, tc);
                    }
                    ui.mark(&format!("level_chip_{i}"), br);
                    if ui.clicked(br) {
                        clicked = Some(i);
                    }
                }
                if let Some(i) = clicked {
                    self.highlight = if self.highlight == Some(i) { None } else { Some(i) };
                    self.recipe_scroll = i.saturating_sub(1);
                    self.tex_dirty = true;
                }
                ui.hud("level_strip");
            }
        }
    }

    fn right_panel(&mut self, ui: &mut Ui, r: Rect) {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_line(r.x, r.y, r.x, r.y + r.h, 1.0, BORDER);
        let x = r.x + 16.0;
        let bw = r.w - 32.0;
        let mut y = r.y + 22.0;
        ui.header("PAINTS FOR THIS PAINTING", x, y);
        y += 10.0;
        let labels: Vec<&str> = Medium::ALL.iter().map(|m| m.label()).collect();
        let cur = Medium::ALL.iter().position(|m| *m == self.medium).unwrap_or(0);
        if let Some(i) = ui.segmented("medium", Rect::new(x, y, bw + 3.0, 30.0), &labels, cur) {
            self.select_medium(Medium::ALL[i]);
        }
        y += 38.0;
        let options: Vec<usize> = (0..self.store.palettes.len()).filter(|&i| self.store.palettes[i].medium == self.medium).collect();
        let names: Vec<String> = options.iter().map(|&i| self.store.palettes[i].name.clone()).collect();
        let label = self
            .current_palette()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("No {} palette yet", self.medium.label().to_lowercase()));
        if let Some(k) = ui.dropdown("palette_select", Rect::new(x, y, bw - 96.0, 30.0), &label, &names) {
            self.palette_idx = Some(options[k]);
            self.mix_dirty = true;
        }
        let edit_label = if options.is_empty() { "Create" } else { "Edit" };
        if ui.button("edit_palettes", Rect::new(x + bw - 90.0, y, 90.0, 30.0), edit_label) {
            self.screen = Screen::Palettes;
            if options.is_empty() {
                self.new_palette(self.medium);
            } else {
                self.edit_palette = self.palette_idx;
                self.select_paint(None);
            }
            // compact layout: straight to the palette's paints
            self.pane_palettes = 1;
            self.pane_scroll = 0.0;
        }
        ui.hud("palette");
        y += 40.0;
        // palette strip
        let mut tip = None;
        if let Some(p) = self.current_palette() {
            let n = p.paints.len();
            let per_row = ((bw + 4.0) / 34.0).floor().max(1.0) as usize;
            for (i, paint) in p.paints.iter().enumerate() {
                let cx = x + 14.0 + (i % per_row) as f32 * 34.0;
                let cy = y + 14.0 + (i / per_row) as f32 * 34.0;
                draw_circle(cx, cy, 14.0, col(paint.color));
                draw_circle_lines(cx, cy, 14.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.2));
                transparency_mark(cx, cy, paint.transparency);
                if vec2(cx, cy).distance(ui.input.mouse) < 14.0 {
                    tip = Some((format!("{}  ({})", paint.title(), paint.transparency.label()), cx, cy));
                }
            }
            if n == 0 {
                ui.text("This palette has no paints yet", x, y + 18.0, 14.0, MUTED);
            }
            y += 34.0 * n.div_ceil(per_row).max(1) as f32 + 6.0;
        } else {
            ui.text("Create a palette for this kind of paint first.", x, y + 14.0, 14.0, MUTED);
            y += 34.0;
        }
        // method: mix on the palette, or glaze (transparent layers)
        ui.text("Method", x, y + 18.0, 14.0, TEXT);
        let labels: Vec<&str> = Method::ALL.iter().map(|m| m.label()).collect();
        let cur = Method::ALL.iter().position(|m| *m == self.method).unwrap_or(0);
        let seg = Rect::new(x + 104.0, y, 160.0_f32.min(bw - 104.0), 26.0);
        if let Some(i) = ui.segmented("method", seg, &labels, cur) {
            if Method::ALL[i] != self.method {
                self.method = Method::ALL[i];
                self.mix_dirty = true;
            }
        }
        ui.hud("method");
        if self.method == Method::Glaze && self.mixer.as_ref().is_some_and(|m| !m.can_glaze()) {
            let hint_x = seg.x + seg.w + 10.0;
            ui.text_fit("no transparent paints", hint_x, y + 18.0, 12.0, WARN, x + bw - hint_x);
        }
        let water = self.medium.uses_water();
        ui.tooltip("method_tip", Rect::new(x, y, seg.x + seg.w - x, 26.0), || {
            let mut lines = vec![
                ("How each colour is reached".to_string(), TEXT),
                ("Mix: the paints are mixed together on the palette.".to_string(), MUTED),
                ("Glaze: transparent layers, each painted once the one below is dry.".to_string(), MUTED),
            ];
            if water {
                lines.push(("A first wash of one paint on the paper, then up to two glazes".to_string(), MUTED));
                lines.push(("of transparent or half-transparent paints thinned with water.".to_string(), MUTED));
            } else {
                lines.push(("An opaque first layer (one paint with white, or the white ground),".to_string(), MUTED));
                lines.push(("then up to two thin glazes of transparent or half-transparent".to_string(), MUTED));
                lines.push(("paints thinned with glazing medium.".to_string(), MUTED));
            }
            lines.push(("Recipes name the layers from the first one up; \"Layers\" sets how many.".to_string(), MUTED));
            lines
        });
        y += 36.0;
        // mixing options; on a narrow panel the checkbox goes on its own line
        let count_label = if self.method == Method::Glaze { "Layers" } else { "Paints per mix" };
        ui.text(count_label, x, y + 18.0, 14.0, TEXT);
        if let Some(i) = ui.segmented("max_paints", Rect::new(x + 104.0, y, 117.0, 26.0), &["1", "2", "3"], self.max_paints as usize - 1) {
            self.max_paints = i as i32 + 1;
            self.mix_dirty = true;
        }
        // in glazing, water or white only lightens the first layer (glazes are thinned anyway)
        let lab = match (self.method, self.medium.uses_water()) {
            (Method::Glaze, _) => "Lighten first layer",
            (_, true) => "Lighten with water",
            (_, false) => "Lighten with white",
        };
        let narrow = bw < 380.0;
        let lr = if narrow { Rect::new(x, y + 34.0, bw, 26.0) } else { Rect::new(x + 236.0, y, bw - 236.0, 26.0) };
        if ui.checkbox("allow_lighten", lr, lab, &mut self.allow_lighten) {
            self.mix_dirty = true;
        }
        y += if narrow { 72.0 } else { 40.0 };
        // mixing model
        let label_r = Rect::new(x, y, 100.0, 26.0);
        ui.text("Mixing model", x, y + 18.0, 14.0, TEXT);
        let labels: Vec<&str> = MixModel::ALL.iter().map(|m| m.label()).collect();
        let cur = MixModel::ALL.iter().position(|m| *m == self.mix_model).unwrap_or(0);
        let seg = Rect::new(x + 104.0, y, 160.0_f32.min(bw - 104.0), 26.0);
        if let Some(i) = ui.segmented("mix_model", seg, &labels, cur) {
            if MixModel::ALL[i] != self.mix_model {
                self.mix_model = MixModel::ALL[i];
                self.mix_dirty = true;
            }
        }
        let tip_r = Rect::new(label_r.x, label_r.y, seg.x + seg.w - label_r.x, 26.0);
        ui.tooltip("mix_model_tip", tip_r, || {
            vec![
                ("How paint mixtures are worked out (Kubelka-Munk)".to_string(), TEXT),
                ("Spectral: each paint's colour becomes a smooth reflectance curve".to_string(), MUTED),
                ("over 31 wavelengths (400-700 nm), mixed wavelength by wavelength:".to_string(), MUTED),
                ("closer to real paint, e.g. a red and its complementary green".to_string(), MUTED),
                ("neutralise to grey, and a red with a blue leans violet.".to_string(), MUTED),
                ("RGB: mixes only red, green and blue; faster, but cruder.".to_string(), MUTED),
            ]
        });
        y += 36.0;

        // clicked spot
        draw_line(x, y, x + bw, y, 1.0, BORDER);
        y += 22.0;
        y = self.picked_section(ui, x, y, bw);
        draw_line(x, y, x + bw, y, 1.0, BORDER);
        y += 22.0;

        // recipes
        let done = self.recipes.iter().filter(|r| r.is_some()).count();
        let total = self.colours.as_ref().map(|c| c.lab.len()).unwrap_or(0);
        let head = if total > 0 && done < total {
            format!("RECIPES FOR {total} COLOUR LEVELS  (mixing {done}/{total})")
        } else {
            format!("RECIPES FOR {total} COLOUR LEVELS")
        };
        ui.header(&head, x, y);
        ui.hud("recipes");
        y += 8.0;
        let list = Rect::new(r.x, y, r.w, r.y + r.h - y);
        ui.mark("recipe_list", list);
        let (Some(c), Some(m)) = (&self.colours, &self.mixer) else {
            if self.img.is_none() {
                ui.text("Open a picture to see its colour levels.", x, y + 20.0, 14.0, MUTED);
            }
            self.draw_tip(ui, tip);
            return;
        };
        // Taller rows, where the paints may wrap to a second line: always on a narrow panel
        // (phone), on a wide one when any recipe does not fit on one line.
        let tw = bw - 64.0;
        let wrap = bw < 380.0 || self.recipes.iter().flatten().any(|rc| !recipe_fits(ui, m, rc, tw));
        let row_h = if wrap { RECIPE_ROW_H_NARROW } else { RECIPE_ROW_H };
        let visible = ((list.h - 4.0) / row_h).floor().max(1.0) as usize;
        let max_scroll = total.saturating_sub(visible);
        let rows = ui.scroll_rows(list, row_h, 1);
        self.recipe_scroll = self.recipe_scroll.saturating_add_signed(rows).min(max_scroll);
        let nvals = self.value_levels as usize;
        let mut clicked = None;
        for (row, i) in (self.recipe_scroll..total).take(visible).enumerate() {
            let rr = Rect::new(x - 6.0, y + row as f32 * row_h, bw + 12.0, row_h - 4.0);
            ui.mark(&format!("recipe_row_{i}"), rr);
            let hl = self.highlight == Some(i);
            rrect(rr, 6.0, if hl { ACCENT_DIM } else if ui.hovered(rr) { PANEL2 } else { BG });
            let sx = rr.x + 6.0;
            let sh = rr.h - 12.0;
            swatch(Rect::new(sx, rr.y + 6.0, 24.0, sh), c.srgb[i]);
            let value_no = self
                .img
                .as_ref()
                .map(|img| imgproc::value_level_of(c.lab[i][0], &img.analysis, nvals) + 1)
                .unwrap_or(0);
            match &self.recipes[i] {
                Some(rc) => {
                    swatch(Rect::new(sx + 27.0, rr.y + 6.0, 24.0, sh), rc.srgb);
                    let tx = sx + 62.0;
                    let info = format!("{}.   {:.0}% of the picture   value {}/{}", i + 1, c.share[i] * 100.0, value_no, nvals);
                    ui.text(&info, tx, rr.y + 17.0, 12.0, MUTED);
                    if !rc.glazes.is_empty() {
                        layer_chips(ui, &format!("recipe_layers_{i}"), m, rc, rr.x + rr.w - 8.0, rr.y + 7.0);
                    }
                    if wrap {
                        recipe_text_wrapped(ui, &format!("recipe_match_{i}"), m, rc, tx, rr.y + 36.0, tw);
                    } else {
                        recipe_text(ui, &format!("recipe_match_{i}"), m, rc, tx, rr.y + 36.0, tw);
                    }
                }
                None => ui.text("mixing...", sx + 62.0, rr.y + 34.0, 14.0, MUTED),
            }
            if ui.clicked(rr) {
                clicked = Some(i);
            }
        }
        if total > visible {
            let track = Rect::new(r.x + r.w - 6.0, y, 3.0, visible as f32 * row_h);
            draw_rectangle(track.x, track.y, track.w, track.h, PANEL2);
            let th = track.h * visible as f32 / total as f32;
            let ty = track.y + (track.h - th) * self.recipe_scroll as f32 / max_scroll.max(1) as f32;
            draw_rectangle(track.x, ty, track.w, th, MUTED);
        }
        if let Some(i) = clicked {
            self.highlight = if self.highlight == Some(i) { None } else { Some(i) };
            self.view = View::Colours;
            self.tex_dirty = true;
        }
        self.draw_tip(ui, tip);
    }

    /// "Clicked spot" header and the recipe for it, from `y` (the header's baseline); returns
    /// the y below it. In the compact layout's Colour levels view, a selected colour level
    /// is shown instead, as its recipe row is in another pane.
    fn picked_section(&mut self, ui: &mut Ui, x: f32, y: f32, bw: f32) -> f32 {
        let tap = if Self::compact() { "Tap" } else { "Click" };
        if Self::compact() && self.view == View::Colours {
            if let (Some(i), Some(c), Some(m)) = (self.highlight, &self.colours, &self.mixer) {
                ui.header(&format!("COLOUR LEVEL {}  ({:.0}% of the picture)", i + 1, c.share[i] * 100.0), x, y);
                let y = y + 10.0;
                swatch(Rect::new(x, y, 44.0, 44.0), c.srgb[i]);
                match self.recipes.get(i).and_then(|r| r.as_ref()) {
                    Some(rc) => {
                        swatch(Rect::new(x + 48.0, y, 44.0, 44.0), rc.srgb);
                        recipe_text_wrapped(ui, &format!("level_match_{i}"), m, rc, x + 100.0, y + 12.0, bw - 100.0);
                    }
                    None => ui.text("mixing...", x + 104.0, y + 26.0, 14.0, MUTED),
                }
                ui.text("level", x + 2.0, y + 58.0, 11.0, MUTED);
                ui.text("mix", x + 60.0, y + 58.0, 11.0, MUTED);
                return y + 68.0;
            }
        }
        ui.header("CLICKED SPOT", x, y);
        let y = y + 10.0;
        ui.hud("picked");
        if let Some(p) = &self.picked {
            swatch(Rect::new(x, y, 44.0, 44.0), p.srgb);
            if let (Some(rc), Some(m)) = (&p.recipe, &self.mixer) {
                swatch(Rect::new(x + 48.0, y, 44.0, 44.0), rc.srgb);
                if Self::compact() || !recipe_fits(ui, m, rc, bw - 104.0) {
                    recipe_text_wrapped(ui, "picked_match", m, rc, x + 100.0, y + 12.0, bw - 100.0);
                } else {
                    recipe_text(ui, "picked_match", m, rc, x + 104.0, y + 16.0, bw - 104.0);
                }
            } else {
                ui.text("No paints to mix with", x + 56.0, y + 26.0, 14.0, MUTED);
            }
            ui.text("picture", x + 2.0, y + 58.0, 11.0, MUTED);
            ui.text("mix", x + 60.0, y + 58.0, 11.0, MUTED);
            y + 68.0
        } else {
            let hint = if self.view == View::Colours {
                format!("{tap} the picture to pick out one colour level.")
            } else {
                format!("{tap} anywhere on the picture to get its recipe.")
            };
            ui.text_fit(&hint, x, y + 16.0, 14.0, MUTED, bw);
            y + 32.0
        }
    }

    fn draw_tip(&self, ui: &Ui, tip: Option<(String, f32, f32)>) {
        let Some((s, cx, cy)) = tip else { return };
        let tw = ui.measure(&s, 14.0);
        let tx = (cx - tw / 2.0).clamp(4.0, screen_width() - tw - 14.0);
        rrect(Rect::new(tx - 6.0, cy + 18.0, tw + 12.0, 24.0), 4.0, BG);
        rrect_lines(Rect::new(tx - 6.0, cy + 18.0, tw + 12.0, 24.0), 4.0, BORDER);
        ui.text(&s, tx, cy + 35.0, 14.0, TEXT);
    }
}

/// Glazing recipe: one small chip per layer with the colour the painting has after that
/// layer (first layer, then each glaze), right-aligned to `right`, top at `y`. Hovering
/// names the layers.
fn layer_chips(ui: &mut Ui, id: &str, m: &Mixer, rc: &Recipe, right: f32, y: f32) {
    const CHIP_W: f32 = 16.0;
    const CHIP_H: f32 = 12.0;
    const GAP: f32 = 4.0;
    let stages = m.stages(rc);
    let n = stages.len() as f32;
    let x0 = right - n * CHIP_W - (n - 1.0) * GAP;
    for (k, c) in stages.iter().enumerate() {
        swatch(Rect::new(x0 + k as f32 * (CHIP_W + GAP), y, CHIP_W, CHIP_H), *c);
    }
    let (pieces, _) = m.part_pieces(rc);
    ui.tooltip(id, Rect::new(x0 - 2.0, y - 2.0, right - x0 + 4.0, CHIP_H + 4.0), || {
        let mut lines = vec![("The colour after each layer, left to right:".to_string(), TEXT)];
        for (k, (p, c)) in pieces.iter().zip(&stages).enumerate() {
            let what = if k == 0 { format!("first layer: {p}") } else { format!("glaze {k}: {}", p.trim_start_matches("then ")) };
            lines.push((format!("{}. {what}  ({})", k + 1, to_hex(*c)), MUTED));
        }
        lines
    });
}
