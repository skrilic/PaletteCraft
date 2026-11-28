//! Palettes screen: list of palettes, the paints in one palette, the paint editor and the pigment library.

use super::*;
use crate::colour::from_hex;
use crate::km::Lighten;
use crate::palette::Paint;
use crate::ui::{ACCENT, ACCENT_DIM, PANEL2, rrect, swatch};

impl App {
    pub(super) fn palettes(&mut self, ui: &mut Ui) {
        let (w, h) = (screen_width(), screen_height());
        if Self::compact() {
            let top = Self::content_top();
            let area = Rect::new(0.0, top, w, h - top);
            match self.pane_palettes {
                0 => {
                    draw_rectangle(area.x, area.y, area.w, area.h, PANEL);
                    let r = Rect::new(0.0, area.y - self.pane_scroll, w, area.h + self.pane_scroll);
                    let end = self.palette_list(ui, r);
                    let mut scroll = self.pane_scroll;
                    ui.scroll_px(area, &mut scroll, end + 24.0 - r.y - area.h);
                    self.pane_scroll = scroll;
                }
                1 => self.paint_list(ui, area),
                _ => {
                    // editor and the whole colour library, scrolled as one pane
                    draw_rectangle(area.x, area.y, area.w, area.h, PANEL);
                    let r = Rect::new(0.0, area.y - self.pane_scroll, w, area.h + self.pane_scroll);
                    let end = self.paint_editor(ui, r);
                    let mut scroll = self.pane_scroll;
                    ui.scroll_px(area, &mut scroll, end + 24.0 - r.y - area.h);
                    self.pane_scroll = scroll;
                }
            }
            return;
        }
        let c1 = Rect::new(0.0, TOP_BAR, 290.0, h - TOP_BAR);
        let c3w = 380.0f32.min(w * 0.3);
        let c2 = Rect::new(c1.w, TOP_BAR, (w - c1.w - c3w).max(300.0), h - TOP_BAR);
        let c3 = Rect::new(c2.x + c2.w, TOP_BAR, w - c2.x - c2.w, h - TOP_BAR);
        draw_rectangle(c1.x, c1.y, c1.w, c1.h, PANEL);
        draw_line(c1.x + c1.w, c1.y, c1.x + c1.w, c1.y + c1.h, 1.0, BORDER);
        draw_rectangle(c3.x, c3.y, c3.w, c3.h, PANEL);
        draw_line(c3.x, c3.y, c3.x, c3.y + c3.h, 1.0, BORDER);
        self.palette_list(ui, c1);
        self.paint_list(ui, c2);
        self.paint_editor(ui, c3);
    }

    /// List of palettes and the selected palette's settings; returns the y where they end.
    fn palette_list(&mut self, ui: &mut Ui, r: Rect) -> f32 {
        let x = r.x + 16.0;
        let bw = r.w - 32.0;
        let mut y = r.y + 22.0;
        ui.header("YOUR PALETTES", x, y);
        ui.hud("palette_list");
        y += 6.0;
        let mut pick = None;
        for m in Medium::ALL {
            ui.text(m.label(), x, y + 16.0, 13.0, MUTED);
            y += 22.0;
            for (i, p) in self.store.palettes.iter().enumerate().filter(|(_, p)| p.medium == m) {
                let rr = Rect::new(x - 6.0, y, bw + 12.0, 32.0);
                ui.mark(&format!("palette_row_{i}"), rr);
                let sel = self.edit_palette == Some(i);
                rrect(rr, 5.0, if sel { ACCENT_DIM } else if ui.hovered(rr) { PANEL2 } else { PANEL });
                ui.text_fit(&p.name, rr.x + 10.0, rr.y + 21.0, 15.0, TEXT, 120.0);
                for (k, paint) in p.paints.iter().take(9).enumerate() {
                    draw_circle(rr.x + 140.0 + k as f32 * 13.0, rr.y + 16.0, 6.0, col(paint.color));
                }
                if self.palette_idx == Some(i) {
                    draw_circle(rr.x + rr.w - 8.0, rr.y + 16.0, 3.0, ACCENT);
                }
                if ui.clicked(rr) {
                    pick = Some(i);
                }
                y += 34.0;
            }
            y += 4.0;
        }
        if let Some(i) = pick {
            self.edit_palette = Some(i);
            self.select_paint(None);
            self.select_lib_colour(None);
            self.confirm_delete = false;
        }
        y += 4.0;
        ui.text("New palette:", x, y + 14.0, 13.0, MUTED);
        y += 22.0;
        let widths = [bw * 0.4, bw * 0.32, bw * 0.28];
        let mut bx = x;
        for (k, m) in Medium::ALL.iter().enumerate() {
            let br = Rect::new(bx, y, widths[k] - 4.0, 28.0);
            bx += widths[k];
            if ui.button(&format!("new_palette_{k}"), br, &format!("+ {}", m.label())) {
                self.new_palette(*m);
            }
        }
        y += 44.0;
        let Some(pi) = self.edit_palette.filter(|&i| i < self.store.palettes.len()) else { return y };
        draw_line(x, y, x + bw, y, 1.0, BORDER);
        y += 22.0;
        ui.header("SELECTED PALETTE", x, y);
        y += 10.0;
        let mut name = self.store.palettes[pi].name.clone();
        if ui.text_field("palette_name", Rect::new(x, y, bw, 30.0), &mut name, "Palette name") {
            self.store.palettes[pi].name = name;
            self.dirty = true;
        }
        y += 38.0;
        let labels: Vec<&str> = Medium::ALL.iter().map(|m| m.label()).collect();
        let cur = Medium::ALL.iter().position(|m| *m == self.store.palettes[pi].medium).unwrap_or(0);
        if let Some(i) = ui.segmented("palette_medium", Rect::new(x, y, bw + 3.0, 28.0), &labels, cur) {
            self.store.palettes[pi].medium = Medium::ALL[i];
            self.palettes_changed();
            self.select_medium(self.medium);
        }
        y += 36.0;
        if ui.button("use_palette", Rect::new(x, y, bw, 30.0), "Use for painting") {
            self.medium = self.store.palettes[pi].medium;
            self.palette_idx = Some(pi);
            self.mix_dirty = true;
            self.screen = Screen::Painting;
        }
        y += 36.0;
        let half = bw / 2.0 - 3.0;
        if ui.button("duplicate_palette", Rect::new(x, y, half, 28.0), "Duplicate") {
            let mut p = self.store.palettes[pi].clone();
            p.name = format!("{} copy", p.name);
            self.store.palettes.insert(pi + 1, p);
            self.edit_palette = Some(pi + 1);
            if let Some(cur) = self.palette_idx.filter(|&c| c > pi) {
                self.palette_idx = Some(cur + 1);
            }
            self.palettes_changed();
        }
        let del_label = if self.confirm_delete { "Click again" } else { "Delete" };
        let del = Rect::new(x + half + 6.0, y, half, 28.0);
        if ui.button("delete_palette", del, del_label) {
            if self.confirm_delete {
                let name = self.store.palettes.remove(pi).name;
                self.say(format!("Deleted palette \"{name}\""));
                self.edit_palette = if self.store.palettes.is_empty() { None } else { Some(pi.saturating_sub(1)) };
                self.palette_idx = match self.palette_idx {
                    Some(c) if c == pi => None,
                    Some(c) if c > pi => Some(c - 1),
                    other => other,
                };
                self.select_medium(self.medium);
                self.select_paint(None);
                self.confirm_delete = false;
                self.palettes_changed();
            } else {
                self.confirm_delete = true;
            }
        } else if ui.input.pressed {
            self.confirm_delete = false;
        }
        let mut y = y + 28.0;
        // compact layout: the top bar has no room for the support button
        if Self::compact() {
            y += 24.0;
            draw_line(x, y, x + bw, y, 1.0, BORDER);
            y += 16.0;
            if ui.support_button("support", Rect::new(x, y, bw, 32.0), "Like the app? Support it on Ko-fi") {
                self.open_support();
            }
            y += 32.0;
        }
        y
    }

    fn paint_list(&mut self, ui: &mut Ui, r: Rect) {
        let x = r.x + 20.0;
        let bw = r.w - 40.0;
        let mut y = r.y + 22.0;
        let Some(pi) = self.edit_palette.filter(|&i| i < self.store.palettes.len()) else {
            ui.text("Create a palette on the left to start.", x, y + 20.0, 16.0, MUTED);
            return;
        };
        let medium = self.store.palettes[pi].medium;
        let title = format!("PAINTS IN \"{}\"  ({})", self.store.palettes[pi].name.to_uppercase(), medium.label().to_lowercase());
        ui.header(&title, x, y);
        ui.hud("paint_list");
        y += 10.0;
        let n = self.store.palettes[pi].paints.len();
        let list_h = (r.h * 0.42).max(200.0);
        let visible = (list_h / 40.0).floor() as usize;
        let list = Rect::new(x, y, bw, visible as f32 * 40.0);
        let rows = ui.scroll_rows(list, 40.0, 1);
        self.paint_scroll = self.paint_scroll.saturating_add_signed(rows).min(n.saturating_sub(visible));
        let mut pick = None;
        for i in (self.paint_scroll..n).take(visible) {
            let p = &self.store.palettes[pi].paints[i];
            let rr = Rect::new(x - 6.0, y, bw + 12.0, 36.0);
            ui.mark(&format!("paint_row_{i}"), rr);
            let sel = self.edit_paint == Some(i);
            rrect(rr, 5.0, if sel { ACCENT_DIM } else if ui.hovered(rr) { PANEL2 } else { BG });
            draw_circle(rr.x + 22.0, rr.y + 18.0, 13.0, col(p.color));
            draw_circle_lines(rr.x + 22.0, rr.y + 18.0, 13.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.2));
            transparency_mark(rr.x + 22.0, rr.y + 18.0, p.transparency);
            ui.text_fit(&p.name, rr.x + 46.0, rr.y + 23.0, 16.0, TEXT, bw * 0.42);
            ui.text_fit(&p.pigment, rr.x + 46.0 + bw * 0.45, rr.y + 23.0, 15.0, ACCENT, bw * 0.22);
            let tl = if bw < 520.0 { p.transparency.short() } else { p.transparency.label() };
            ui.text(tl, rr.x + rr.w - ui.measure(tl, 14.0) - 12.0, rr.y + 23.0, 14.0, MUTED);
            if ui.clicked(rr) {
                pick = Some(i);
            }
            y += 40.0;
        }
        if n == 0 {
            let lines: &[&str] = if Self::compact() {
                &["No paints yet: add some under \"Edit & library\",", "or add a blank paint and fill it in there."]
            } else {
                &["No paints yet: pick some from the pigment library on the right,", "or add a blank paint and type its name and pigment code."]
            };
            for (k, l) in lines.iter().enumerate() {
                ui.text_fit(l, x, y + 20.0 + 20.0 * k as f32, 15.0, MUTED, bw);
            }
        }
        if let Some(i) = pick {
            self.select_paint(Some(i));
        }
        let mut y = list.y + list.h + 6.0;
        let b4 = (bw - 18.0) / 4.0;
        // short labels when the buttons are narrow (phones)
        let [add_l, remove_l, up_l, down_l] =
            if b4 < 110.0 { ["+ Blank", "Remove", "Up", "Down"] } else { ["+ Blank paint", "Remove", "Move up", "Move down"] };
        if ui.button("add_paint", Rect::new(x, y, b4, 28.0), add_l) {
            let pal = &mut self.store.palettes[pi];
            pal.paints.push(Paint {
                name: "New paint".into(),
                pigment: "P".into(),
                transparency: Transparency::HalfTransparent,
                color: [128, 128, 128],
                strength: 1.0,
            });
            let i = pal.paints.len() - 1;
            self.select_paint(Some(i));
            self.paint_scroll = (i + 1).saturating_sub(visible);
            self.palettes_changed();
        }
        let sel = self.edit_paint.filter(|&i| i < n);
        if ui.button("remove_paint", Rect::new(x + b4 + 6.0, y, b4, 28.0), remove_l) {
            if let Some(i) = sel {
                self.store.palettes[pi].paints.remove(i);
                self.select_paint(None);
                self.palettes_changed();
            }
        }
        if ui.button("paint_up", Rect::new(x + 2.0 * (b4 + 6.0), y, b4, 28.0), up_l) {
            if let Some(i) = sel.filter(|&i| i > 0) {
                self.store.palettes[pi].paints.swap(i, i - 1);
                self.select_paint(Some(i - 1));
                self.palettes_changed();
            }
        }
        if ui.button("paint_down", Rect::new(x + 3.0 * (b4 + 6.0), y, b4, 28.0), down_l) {
            if let Some(i) = sel.filter(|&i| i + 1 < n) {
                self.store.palettes[pi].paints.swap(i, i + 1);
                self.select_paint(Some(i + 1));
                self.palettes_changed();
            }
        }
        y += 50.0;
        self.mixing_chart(ui, Rect::new(x, y, bw, r.y + r.h - y - 10.0), pi);
    }

    fn mixing_chart(&mut self, ui: &mut Ui, r: Rect, pi: usize) {
        let pal = &self.store.palettes[pi];
        let n = pal.paints.len();
        if n == 0 {
            return;
        }
        let m = Mixer::new(pal, self.mix_model);
        let head = match self.mix_model {
            MixModel::Spectral => "MIXING CHART  (1 : 1 mixes, spectral Kubelka-Munk)",
            MixModel::Rgb => "MIXING CHART  (1 : 1 mixes, RGB Kubelka-Munk)",
        };
        ui.header(head, r.x, r.y);
        ui.hud("mixing_chart");
        let water = pal.medium.uses_water();
        let tint_steps: Vec<Lighten> = if water {
            [1.0, 0.55, 0.3, 0.16, 0.075].iter().map(|&l| Lighten::Water(l)).collect()
        } else {
            [0u32, 1, 3, 8, 24].iter().map(|&n| if n == 0 { Lighten::None } else { Lighten::White(n) }).collect()
        };
        let cell = ((r.w - 170.0) / (n + 1) as f32).min((r.h - 50.0) / (n + 1) as f32).clamp(8.0, 34.0);
        let ox = r.x;
        let oy = r.y + 14.0;
        let grid_w = cell * (n + 1) as f32;
        if water {
            draw_rectangle(ox + cell, oy + cell, cell * n as f32, cell * n as f32, col(m.paper_srgb()));
        }
        let mut tip = None;
        for i in 0..n {
            draw_circle(ox + cell * (i as f32 + 1.5), oy + cell * 0.5, cell * 0.36, col(pal.paints[i].color));
            draw_circle(ox + cell * 0.5, oy + cell * (i as f32 + 1.5), cell * 0.36, col(pal.paints[i].color));
            for j in 0..n {
                let c = if i == j { m.mix_srgb(&[(i, 1)], Lighten::None) } else { m.mix_srgb(&[(i, 1), (j, 1)], Lighten::None) };
                let cr = Rect::new(ox + cell * (j as f32 + 1.0) + 1.0, oy + cell * (i as f32 + 1.0) + 1.0, cell - 2.0, cell - 2.0);
                draw_rectangle(cr.x, cr.y, cr.w, cr.h, col(c));
                if cr.contains(ui.input.mouse) {
                    draw_rectangle_lines(cr.x - 1.0, cr.y - 1.0, cr.w + 2.0, cr.h + 2.0, 2.0, WHITE);
                    tip = Some(if i == j { pal.paints[i].title() } else { format!("{} + {}", pal.paints[i].name, pal.paints[j].name) });
                }
            }
        }
        // tint ladder: each paint lightened with water or white
        let lx = ox + grid_w + 24.0;
        let tc = ((r.x + r.w - lx) / tint_steps.len() as f32).clamp(8.0, cell.max(20.0));
        ui.text(if water { "with water  ->" } else { "with white  ->" }, lx, oy + cell * 0.6, 12.0, MUTED);
        if water {
            draw_rectangle(lx, oy + cell, tc * tint_steps.len() as f32, cell * n as f32, col(m.paper_srgb()));
        }
        for i in 0..n {
            for (k, l) in tint_steps.iter().enumerate() {
                let c = m.mix_srgb(&[(i, 1)], *l);
                draw_rectangle(lx + k as f32 * tc + 1.0, oy + cell * (i as f32 + 1.0) + 1.0, tc - 2.0, cell - 2.0, col(c));
            }
        }
        let ty = oy + grid_w + 22.0;
        let hint = if Self::compact() { "Tap a square to see which paints it mixes." } else { "Point at a square to see which paints it mixes." };
        let text = tip.clone().unwrap_or_else(|| hint.into());
        ui.text_fit(&text, ox, ty, 14.0, if tip.is_none() { MUTED } else { TEXT }, r.w);
    }

    /// Right column: the editor (a palette paint, or a colour of the artist's library), then
    /// the colour library. Returns the y where it ends.
    fn paint_editor(&mut self, ui: &mut Ui, r: Rect) -> f32 {
        let x = r.x + 16.0;
        let bw = r.w - 32.0;
        let mut y = r.y + 22.0;
        if let Some(li) = self.lib_edit.filter(|&i| i < self.store.library.len()) {
            y = self.lib_colour_editor(ui, x, y, bw, li);
        } else {
            self.lib_edit = None;
            self.lib_unsaved = None;
            y = self.palette_paint_editor(ui, x, y, bw);
        }
        draw_line(x, y, x + bw, y, 1.0, BORDER);
        y += 22.0;
        self.library_list(ui, x, y, bw, r.y + r.h)
    }

    /// Editor of the selected palette paint; returns the y below it.
    fn palette_paint_editor(&mut self, ui: &mut Ui, x: f32, mut y: f32, bw: f32) -> f32 {
        let pi = self.edit_palette.filter(|&i| i < self.store.palettes.len());
        let sel = pi.and_then(|p| self.edit_paint.filter(|&i| i < self.store.palettes[p].paints.len()).map(|i| (p, i)));
        ui.header("PAINT", x, y);
        y += 10.0;
        let Some((p, i)) = sel else {
            let msg = if pi.is_some() { "Select a paint in the list to edit it." } else { "Select a palette first." };
            ui.text(msg, x, y + 18.0, 14.0, MUTED);
            return y + 36.0;
        };
        let mut changed = false;
        let paint = &mut self.store.palettes[p].paints[i];
        ui.text("Name", x, y + 20.0, 14.0, MUTED);
        changed |= ui.text_field("paint_name", Rect::new(x + 76.0, y, bw - 76.0, 30.0), &mut paint.name, "e.g. Crimson");
        y += 36.0;
        ui.text("Pigment", x, y + 20.0, 14.0, MUTED);
        changed |= ui.text_field("paint_code", Rect::new(x + 76.0, y, bw - 76.0, 30.0), &mut paint.pigment, "e.g. PR206");
        y += 40.0;
        changed |= transparency_choice(ui, "paint_transparency", Rect::new(x, y, bw + 3.0, 28.0), &mut paint.transparency);
        y += 38.0;
        let (ny, c) = colour_fields(ui, "paint", x, y, bw, &mut paint.color, &mut paint.strength, paint.transparency, &mut self.hex_edit);
        y = ny;
        changed |= c;
        if changed {
            self.palettes_changed();
        }
        // keep this paint as a colour of the artist's own library
        if ui.button("save_to_library", Rect::new(x, y, bw, 28.0), "Save to my colour library") {
            let medium = self.store.palettes[p].medium;
            let colour = LibColour::from_paint(&self.store.palettes[p].paints[i], &self.new_colour_brand(), vec![medium]);
            let name = colour.name.clone();
            self.add_lib_colour(colour);
            self.say(format!("Saved {name} to your colour library: set its brand and use above"));
        }
        y + 40.0
    }

    /// Editor of colour `li` of the artist's library; returns the y below it.
    fn lib_colour_editor(&mut self, ui: &mut Ui, x: f32, mut y: f32, bw: f32, li: usize) -> f32 {
        ui.header("COLOUR IN MY LIBRARY", x, y);
        ui.hud("lib_colour_editor");
        y += 10.0;
        let before = self.store.library[li].clone();
        let mut c = before.clone();
        ui.text("Name", x, y + 20.0, 14.0, MUTED);
        ui.text_field("libc_name", Rect::new(x + 76.0, y, bw - 76.0, 30.0), &mut c.name, "e.g. Opera Rose");
        y += 36.0;
        ui.text("Pigment", x, y + 20.0, 14.0, MUTED);
        ui.text_field("libc_code", Rect::new(x + 76.0, y, bw - 76.0, 30.0), &mut c.pigment, "e.g. PR122 or PB29+PV19");
        y += 36.0;
        ui.text("Brand", x, y + 20.0, 14.0, MUTED);
        let field = Rect::new(x + 76.0, y, bw - 76.0 - 34.0, 30.0);
        ui.text_field("libc_brand", field, &mut c.brand, "brand or library name");
        let brands = self.library_brands();
        let pick_r = Rect::new(x + bw - 30.0, y, 30.0, 30.0);
        let anchor = Rect::new(field.x, y, bw - 76.0, 30.0);
        let own: Vec<String> = brands.iter().filter(|b| !palette::same_brand(b, palette::GENERAL_BRAND)).cloned().collect();
        let mut suggestions = own.clone();
        if !suggestions.iter().any(|b| palette::same_brand(b, palette::DEFAULT_BRAND)) {
            suggestions.push(palette::DEFAULT_BRAND.to_string());
        }
        let items = suggestions.clone();
        if let Some(Pick::Item(k)) = ui.menu_button("libc_brand_pick", pick_r, anchor, move || (items, 0)) {
            c.brand = suggestions[k].clone();
        }
        y += 40.0;
        transparency_choice(ui, "libc_transparency", Rect::new(x, y, bw + 3.0, 28.0), &mut c.transparency);
        y += 36.0;
        // kinds of paint it is for; at least one stays ticked
        ui.text("For", x, y + 18.0, 14.0, MUTED);
        // three equal columns where every label fits; on a narrow panel (phone) each column
        // takes its own label's width plus an equal share of what is left
        let cw = (bw - 40.0) / 3.0;
        let widths: Vec<f32> = Medium::ALL.iter().map(|m| 24.0 + ui.measure(m.label(), 15.0) + 4.0).collect();
        let col_w: Vec<f32> = if widths.iter().all(|w| *w <= cw) {
            vec![cw; widths.len()]
        } else {
            let spare = ((bw - 40.0 - widths.iter().sum::<f32>()) / widths.len() as f32).max(6.0);
            widths.iter().map(|w| w + spare).collect()
        };
        let mut cx = x + 40.0;
        for (k, m) in Medium::ALL.iter().enumerate() {
            let mut on = c.has_use(*m);
            let cr = Rect::new(cx, y, col_w[k] - 4.0, 26.0);
            cx += col_w[k];
            if ui.checkbox(&format!("libc_use_{k}"), cr, m.label(), &mut on) {
                if on {
                    c.uses.push(*m);
                    c.uses.sort_by_key(|u| Medium::ALL.iter().position(|v| v == u));
                } else if c.uses.len() > 1 {
                    c.uses.retain(|u| u != m);
                } else {
                    self.say("A colour is for at least one kind of paint");
                }
            }
        }
        y += 34.0;
        let (ny, _) = colour_fields(ui, "libc", x, y, bw, &mut c.color, &mut c.strength, c.transparency, &mut self.hex_edit);
        y = ny;
        if c != before {
            self.lib_last_brand = c.brand.trim().to_string();
            self.store.library[li] = c;
            self.dirty = true;
        }
        // add to the palette being edited, delete (click twice), close; a colour just added
        // offers Cancel (take it out again) and Save instead
        let b3 = (bw - 12.0) / 3.0;
        let pi = self.edit_palette.filter(|&i| i < self.store.palettes.len());
        if ui.button("libc_add", Rect::new(x, y, b3, 28.0), "Add to palette") {
            match pi {
                Some(p) => {
                    self.lib_unsaved = None;
                    self.add_to_palette(p, &self.store.library[li].clone());
                }
                None => self.say("Select a palette first"),
            }
        }
        if self.lib_unsaved == Some(li) {
            if ui.button("libc_cancel", Rect::new(x + b3 + 6.0, y, b3, 28.0), "Cancel") {
                let gone = self.store.library.remove(li);
                self.lib_unsaved = None;
                self.dirty = true;
                self.select_lib_colour(None);
                self.say(format!("Cancelled: {} was not added to your colour library", gone.name));
                return y + 40.0;
            }
            if ui.button("libc_save", Rect::new(x + 2.0 * (b3 + 6.0), y, b3, 28.0), "Save") {
                let name = self.store.library[li].name.clone();
                self.lib_unsaved = None;
                self.select_lib_colour(None);
                self.say(format!("Saved {name} in your colour library"));
            }
            return y + 40.0;
        }
        let del_label = if self.lib_confirm_delete { "Click again" } else { "Delete" };
        let del_r = Rect::new(x + b3 + 6.0, y, b3, 28.0);
        if ui.button("libc_delete", del_r, del_label) {
            if self.lib_confirm_delete {
                let gone = self.store.library.remove(li);
                self.dirty = true;
                self.select_lib_colour(None);
                self.say(format!("Deleted {} from your colour library (palettes keep their copies)", gone.name));
                return y + 40.0;
            }
            self.lib_confirm_delete = true;
        } else if ui.input.pressed && !ui.hovered(del_r) {
            self.lib_confirm_delete = false;
        }
        if ui.button("libc_done", Rect::new(x + 2.0 * (b3 + 6.0), y, b3, 28.0), "Done") {
            self.select_lib_colour(None);
        }
        y + 40.0
    }

    /// Adds a copy of library colour `c` to palette `p` and selects it.
    fn add_to_palette(&mut self, p: usize, c: &LibColour) {
        let pal = &mut self.store.palettes[p];
        pal.paints.push(c.to_paint());
        let i = pal.paints.len() - 1;
        let (name, medium) = (pal.name.clone(), pal.medium);
        if self.lib_edit.is_none() {
            self.select_paint(Some(i));
        } else {
            self.edit_paint = None;
        }
        self.paint_scroll = i;
        self.palettes_changed();
        let note = if c.has_use(medium) { String::new() } else { format!(" (it is made for {})", c.uses_label().to_lowercase()) };
        self.say(format!("Added {} {} to {}{note}", c.name, c.pigment, name));
    }

    /// The colour library: search, filters by use and brand, grouping, and the list (general
    /// colours fixed, the artist's own with an Edit button). Returns the y where it ends.
    fn library_list(&mut self, ui: &mut Ui, x: f32, mut y: f32, bw: f32, bottom: f32) -> f32 {
        let compact = Self::compact();
        let head = if compact { "COLOUR LIBRARY  (tap to add)" } else { "COLOUR LIBRARY  (click to add)" };
        ui.text_fit(head, x, y, 13.0, MUTED, bw - 128.0);
        ui.hud("colour_library");
        if ui.button("lib_new", Rect::new(x + bw - 120.0, y - 18.0, 120.0, 26.0), "+ New colour") {
            let colour = LibColour {
                name: "New colour".into(),
                pigment: "P".into(),
                transparency: Transparency::HalfTransparent,
                color: [128, 128, 128],
                strength: 1.0,
                brand: self.new_colour_brand(),
                uses: self.new_colour_uses(),
            };
            self.add_lib_colour(colour);
        }
        y += 12.0;
        // search and brand filter
        let sw = (bw * 0.55).floor();
        if ui.text_field("lib_search", Rect::new(x, y, sw - 6.0, 30.0), &mut self.lib_search, "Search name, code") {
            self.lib_scroll = 0;
        }
        let brands = self.library_brands();
        let mut names = vec!["All brands".to_string()];
        names.extend(brands.iter().cloned());
        let label = self.lib_brand.clone().unwrap_or_else(|| "All brands".into());
        if let Some(k) = ui.dropdown("lib_brand", Rect::new(x + sw, y, bw - sw, 30.0), &label, &names) {
            self.lib_brand = if k == 0 { None } else { Some(names[k].clone()) };
            self.lib_scroll = 0;
        }
        y += 36.0;
        // use filter
        let uses = ["All", "Watercolor", "Acrylic", "Oil"];
        let cur = self.lib_use.and_then(|m| Medium::ALL.iter().position(|x| *x == m)).map_or(0, |i| i + 1);
        if let Some(k) = ui.segmented("lib_use", Rect::new(x, y, bw + 3.0, 26.0), &uses, cur) {
            self.lib_use = if k == 0 { None } else { Some(Medium::ALL[k - 1]) };
            self.lib_scroll = 0;
        }
        y += 32.0;
        // grouping
        ui.text("Group by", x, y + 18.0, 14.0, MUTED);
        let labels: Vec<&str> = LibGroup::ALL.iter().map(|g| g.label()).collect();
        let cur = LibGroup::ALL.iter().position(|g| *g == self.lib_group).unwrap_or(0);
        if let Some(k) = ui.segmented("lib_group", Rect::new(x + 76.0, y, bw - 73.0, 26.0), &labels, cur) {
            self.lib_group = LibGroup::ALL[k];
            self.lib_scroll = 0;
        }
        y += 34.0;
        let rows = self.library_rows();
        let row_h = 28.0;
        // compact: the whole pane scrolls, so every row is drawn
        let visible = if compact { rows.len().max(1) } else { ((bottom - y - 8.0) / row_h).floor().max(1.0) as usize };
        let list = Rect::new(x, y, bw, visible as f32 * row_h);
        if !compact {
            let scrolled = ui.scroll_rows(list, row_h, 2);
            self.lib_scroll = self.lib_scroll.saturating_add_signed(scrolled);
        }
        if self.lib_reveal {
            self.lib_reveal = false;
            let at = rows.iter().position(|r| matches!(r, LibRow::Colour { custom: Some(c), .. } if Some(*c) == self.lib_edit));
            if let Some(at) = at.filter(|&a| a < self.lib_scroll || a >= self.lib_scroll + visible) {
                self.lib_scroll = at.saturating_sub(1);
            }
        }
        self.lib_scroll = if compact { 0 } else { self.lib_scroll.min(rows.len().saturating_sub(visible)) };
        if rows.iter().all(|r| matches!(r, LibRow::Header(..))) {
            ui.text_fit("No colours match the search and filters.", x, y + 20.0, 14.0, MUTED, bw);
        }
        let mut add = None;
        let mut edit = None;
        let mut k = 0;
        let edit_w = 46.0;
        for (row, item) in rows.iter().skip(self.lib_scroll).take(visible).enumerate() {
            let rr = Rect::new(x - 6.0, y + row as f32 * row_h, bw + 12.0, row_h - 2.0);
            match item {
                LibRow::Header(name, n) => {
                    ui.text_fit(&format!("{}  ({n})", name.to_uppercase()), rr.x + 6.0, rr.y + 19.0, 12.0, ACCENT, bw - 10.0);
                    draw_line(rr.x + 6.0, rr.y + rr.h - 1.0, rr.x + rr.w - 6.0, rr.y + rr.h - 1.0, 1.0, BORDER);
                }
                LibRow::Colour { custom, colour } => {
                    ui.mark(&format!("lib_row_{k}"), rr);
                    let editing = custom.is_some() && *custom == self.lib_edit;
                    if editing {
                        rrect(rr, 4.0, ACCENT_DIM);
                    } else if ui.hovered(rr) {
                        rrect(rr, 4.0, PANEL2);
                    }
                    draw_circle(rr.x + 16.0, rr.y + 13.0, 9.0, col(colour.color));
                    transparency_mark(rr.x + 16.0, rr.y + 13.0, colour.transparency);
                    let right = if custom.is_some() { edit_w + 6.0 } else { 0.0 };
                    ui.text_fit(&colour.name, rr.x + 34.0, rr.y + 18.0, 14.0, TEXT, bw * 0.46);
                    // the brand goes beside the code when the list is not grouped by brand
                    let code = if self.lib_group == LibGroup::Brand {
                        colour.pigment.clone()
                    } else {
                        format!("{}  {}", colour.pigment, colour.brand_label())
                    };
                    let cx = rr.x + 34.0 + bw * 0.48;
                    ui.text_fit(&code, cx, rr.y + 18.0, 13.0, ACCENT, rr.x + rr.w - 30.0 - right - cx - 4.0);
                    let tl = colour.transparency.short();
                    ui.text(tl, rr.x + rr.w - 26.0 - right, rr.y + 18.0, 13.0, MUTED);
                    let mut on_edit = false;
                    if let Some(ci) = custom {
                        let er = Rect::new(rr.x + rr.w - edit_w - 2.0, rr.y + 2.0, edit_w, rr.h - 4.0);
                        on_edit = er.contains(ui.input.mouse);
                        if ui.button(&format!("lib_edit_{k}"), er, "Edit") {
                            edit = Some(*ci);
                        }
                    }
                    if !on_edit && ui.clicked(rr) {
                        add = Some(colour.clone());
                    }
                    k += 1;
                }
            }
        }
        if let Some(ci) = edit {
            self.select_lib_colour(Some(ci));
        }
        if let Some(c) = add {
            match self.edit_palette.filter(|&i| i < self.store.palettes.len()) {
                Some(p) => self.add_to_palette(p, &c),
                None => self.say("Select a palette first"),
            }
        }
        if !compact && rows.len() > visible {
            let track = Rect::new(x + bw + 9.0, list.y, 3.0, list.h);
            draw_rectangle(track.x, track.y, track.w, track.h, PANEL2);
            let th = track.h * visible as f32 / rows.len() as f32;
            let ty = track.y + (track.h - th) * self.lib_scroll as f32 / rows.len().saturating_sub(visible).max(1) as f32;
            draw_rectangle(track.x, ty, track.w, th, MUTED);
        }
        list.y + list.h
    }
}

/// Transparent / Half-transparent / Opaque choice; true when changed.
fn transparency_choice(ui: &mut Ui, id: &str, r: Rect, t: &mut Transparency) -> bool {
    let labels: Vec<&str> = Transparency::ALL.iter().map(|t| t.label()).collect();
    let cur = Transparency::ALL.iter().position(|x| x == t).unwrap_or(1);
    match ui.segmented(id, r, &labels, cur) {
        Some(k) if Transparency::ALL[k] != *t => {
            *t = Transparency::ALL[k];
            true
        }
        _ => false,
    }
}

/// Swatch, hex field, R/G/B sliders and tinting strength of a paint or library colour; ids
/// start with `id`. Returns the y below them and whether anything changed.
#[allow(clippy::too_many_arguments)]
fn colour_fields(
    ui: &mut Ui,
    id: &str,
    x: f32,
    mut y: f32,
    bw: f32,
    color: &mut [u8; 3],
    strength: &mut f32,
    transparency: Transparency,
    hex_edit: &mut String,
) -> (f32, bool) {
    let mut changed = false;
    swatch(Rect::new(x, y, 60.0, 60.0), *color);
    transparency_mark(x + 30.0, y + 30.0, transparency);
    ui.text("Colour", x + 72.0, y + 20.0, 14.0, MUTED);
    if ui.text_field(&format!("{id}_hex"), Rect::new(x + 130.0, y, bw - 130.0, 30.0), hex_edit, "#RRGGBB") {
        if let Some(c) = from_hex(hex_edit) {
            *color = c;
            changed = true;
        }
    }
    ui.text("masstone, full strength", x + 72.0, y + 52.0, 12.0, MUTED);
    y += 70.0;
    let mut rgb = color.map(|v| v as i32);
    for (k, name) in ["R", "G", "B"].iter().enumerate() {
        ui.text(name, x, y + 16.0, 14.0, MUTED);
        ui.slider_int(&format!("{id}_rgb_{k}"), Rect::new(x + 14.0, y, bw - 54.0, 22.0), &mut rgb[k], 0, 255);
        ui.text(&rgb[k].to_string(), x + bw - 30.0, y + 16.0, 13.0, TEXT);
        y += 26.0;
    }
    let new_rgb = rgb.map(|v| v as u8);
    if new_rgb != *color {
        *color = new_rgb;
        *hex_edit = to_hex(new_rgb);
        changed = true;
    }
    ui.text("Tinting strength", x, y + 16.0, 14.0, MUTED);
    let s = format!("{:.1}", strength);
    ui.text(&s, x + bw - ui.measure(&s, 14.0), y + 16.0, 14.0, ACCENT);
    if ui.slider_f(&format!("{id}_strength"), Rect::new(x + 112.0, y, bw - 150.0, 22.0), strength, 0.2, 3.0) {
        *strength = (*strength * 10.0).round() / 10.0;
        changed = true;
    }
    (y + 34.0, changed)
}
