//! Small immediate-mode UI on top of macroquad: buttons, sliders, text fields, dropdowns, tooltips.
//! Every widget records its rectangle by id so the self-test can click it like a person.

use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};

pub const BG: Color = Color::new(0.105, 0.11, 0.125, 1.0);
pub const PANEL: Color = Color::new(0.145, 0.15, 0.17, 1.0);
pub const PANEL2: Color = Color::new(0.185, 0.19, 0.215, 1.0);
pub const PANEL3: Color = Color::new(0.235, 0.24, 0.27, 1.0);
pub const BORDER: Color = Color::new(0.30, 0.31, 0.35, 1.0);
pub const TEXT: Color = Color::new(0.92, 0.92, 0.94, 1.0);
pub const MUTED: Color = Color::new(0.62, 0.63, 0.68, 1.0);
pub const ACCENT: Color = Color::new(0.93, 0.62, 0.27, 1.0);
pub const ACCENT_DIM: Color = Color::new(0.42, 0.30, 0.17, 1.0);
pub const GOOD: Color = Color::new(0.45, 0.80, 0.52, 1.0);
pub const WARN: Color = Color::new(0.95, 0.72, 0.35, 1.0);
pub const BAD: Color = Color::new(0.93, 0.45, 0.42, 1.0);

/// How far (px) the pointer may move between press and release and still count as a tap.
pub const TAP_SLOP: f32 = 10.0;

#[derive(Default, Clone, Debug)]
pub struct Input {
    pub mouse: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub wheel: f32,
    pub chars: Vec<char>,
    pub backspace: bool,
    pub enter: bool,
    pub escape: bool,
}

impl Input {
    pub fn from_system() -> Self {
        let (mx, my) = mouse_position();
        let mut chars = Vec::new();
        while let Some(c) = get_char_pressed() {
            if !c.is_control() {
                chars.push(c);
            }
        }
        let (_, wy) = mouse_wheel();
        Input {
            mouse: vec2(mx, my),
            down: is_mouse_button_down(MouseButton::Left),
            pressed: is_mouse_button_pressed(MouseButton::Left),
            wheel: wy.clamp(-3.0, 3.0),
            chars,
            backspace: is_key_pressed(KeyCode::Backspace),
            enter: is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter),
            escape: is_key_pressed(KeyCode::Escape),
        }
    }
}

/// Open list of choices. An item may hold a tab: the text after it is shown muted, as a note.
pub struct Popup {
    pub id: String,
    pub anchor: Rect,
    pub items: Vec<String>,
    /// The first `removable` items have a × at the right that removes them (see `Pick`).
    pub removable: usize,
    /// Width that fits the items (at least the anchor's).
    pub w: f32,
}

/// What was done with a popup item.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pick {
    /// The item was chosen; the popup closes.
    Item(usize),
    /// Its × was tapped; the popup stays open (refresh it with `Ui::set_popup_items`).
    Remove(usize),
}

/// Width of the × area at the right of a removable popup item.
const REMOVE_W: f32 = 34.0;

/// Help box shown while the mouse is over `anchor`; drawn on top of everything in `end()`.
struct Tooltip {
    anchor: Rect,
    lines: Vec<(String, Color)>,
}

pub struct Ui {
    pub input: Input,
    pub font: Option<Font>,
    pub active: Option<String>,
    pub focus: Option<String>,
    pub rects: HashMap<String, Rect>,
    pub prev_rects: HashMap<String, Rect>,
    /// Names of on-screen information drawn this frame (read by the self-test).
    pub hud: HashSet<String>,
    pub prev_hud: HashSet<String>,
    popup: Option<Popup>,
    popup_result: Option<(String, Pick)>,
    tooltip: Option<Tooltip>,
    swallow: bool,
    block: Option<Rect>,
    pub time: f32,
    /// Where the button or finger went down, while it is held.
    press_at: Option<Vec2>,
    /// The pointer moved more than `TAP_SLOP` since the press (a drag, not a tap).
    dragged: bool,
    /// Released this frame without dragging: buttons react to this, so dragging to scroll
    /// a list on a touch screen does not press the buttons under the finger.
    tap: bool,
    tap_from: Vec2,
    mouse_delta: Vec2,
    row_drag: f32,
    /// Nothing above this y reacts to the pointer (content scrolled under the top bars).
    clip_top: f32,
    /// A dialog lies over what is drawn now: none of it reacts (see `set_covered`).
    covered: bool,
}

impl Ui {
    pub fn new(font: Option<Font>) -> Self {
        Ui {
            input: Input::default(),
            font,
            active: None,
            focus: None,
            rects: HashMap::new(),
            prev_rects: HashMap::new(),
            hud: HashSet::new(),
            prev_hud: HashSet::new(),
            popup: None,
            popup_result: None,
            tooltip: None,
            swallow: false,
            block: None,
            time: 0.0,
            press_at: None,
            dragged: false,
            tap: false,
            tap_from: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            row_drag: 0.0,
            clip_top: 0.0,
            covered: false,
        }
    }

    pub fn begin(&mut self, input: Input, dt: f32) {
        let prev_mouse = self.input.mouse;
        self.input = input;
        self.mouse_delta = if self.input.pressed { Vec2::ZERO } else { self.input.mouse - prev_mouse };
        if self.input.pressed {
            self.press_at = Some(self.input.mouse);
            self.dragged = false;
            self.row_drag = 0.0;
        }
        if self.press_at.is_some_and(|p| p.distance(self.input.mouse) > TAP_SLOP) {
            self.dragged = true;
        }
        self.tap = false;
        if !self.input.down {
            if let Some(p) = self.press_at.take() {
                self.tap = !self.dragged;
                self.tap_from = p;
            }
        }
        self.clip_top = 0.0;
        self.covered = false;
        self.time += dt;
        self.prev_rects = std::mem::take(&mut self.rects);
        self.prev_hud = std::mem::take(&mut self.hud);
        self.tooltip = None;
        self.block = self.popup.as_ref().map(|p| popup_rect(p));
        // While a popup is open, a press or tap goes to the popup only.
        self.swallow = self.popup.is_some() && (self.input.pressed || self.tap);
        if !self.input.down && !self.input.pressed {
            self.active = None;
        }
    }

    pub fn end(&mut self) {
        if let Some(p) = &self.popup {
            let r = popup_rect(p);
            draw_rectangle(r.x + 3.0, r.y + 4.0, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
            draw_rectangle(r.x, r.y, r.w, r.h, PANEL2);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
            let mut chosen = None;
            for (i, item) in p.items.iter().enumerate() {
                let ir = Rect::new(r.x, r.y + 4.0 + i as f32 * 28.0, r.w, 28.0);
                self.rects.insert(format!("{}#{}", p.id, i), ir);
                let removable = i < p.removable;
                let xr = Rect::new(ir.x + ir.w - REMOVE_W, ir.y, REMOVE_W, ir.h);
                if ir.contains(self.input.mouse) {
                    draw_rectangle(ir.x + 2.0, ir.y, ir.w - 4.0, ir.h, PANEL3);
                    if self.tap {
                        chosen = Some(if removable && xr.contains(self.input.mouse) { Pick::Remove(i) } else { Pick::Item(i) });
                    }
                }
                if removable {
                    self.rects.insert(format!("{}#{}:remove", p.id, i), xr);
                    let over = xr.contains(self.input.mouse);
                    let (cx, cy) = (xr.x + xr.w / 2.0, xr.y + xr.h / 2.0);
                    if over {
                        draw_circle(cx, cy, 10.0, PANEL2);
                    }
                    let c = if over { TEXT } else { MUTED };
                    draw_line(cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0, 1.5, c);
                    draw_line(cx - 4.0, cy + 4.0, cx + 4.0, cy - 4.0, 1.5, c);
                }
                let max_w = ir.w - 20.0 - if removable { REMOVE_W - 6.0 } else { 0.0 };
                match item.split_once('\t') {
                    Some((main, note)) => {
                        let mw = self.measure(main, 15.0).min(max_w);
                        self.text_fit(main, ir.x + 10.0, ir.y + 19.0, 15.0, TEXT, max_w);
                        if max_w - mw > 40.0 {
                            self.text_fit(note, ir.x + 10.0 + mw + 12.0, ir.y + 19.0, 13.0, MUTED, max_w - mw - 12.0);
                        }
                    }
                    None => self.text_fit(item, ir.x + 10.0, ir.y + 19.0, 15.0, TEXT, max_w),
                }
            }
            if self.tap {
                let keep_open = matches!(chosen, Some(Pick::Remove(_)));
                if let Some(pick) = chosen {
                    self.popup_result = Some((p.id.clone(), pick));
                }
                if !keep_open {
                    self.popup = None;
                }
            }
        }
        if let Some(t) = &self.tooltip {
            self.draw_tooltip(t);
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::frame_end();
    }

    fn draw_tooltip(&self, t: &Tooltip) {
        // Shrunk to fit narrow screens (phones).
        let sw = screen_width();
        let full_w = self.tooltip_width(t, 14.0, 14.0) + 20.0;
        let k = ((sw - 8.0) / full_w).clamp(0.6, 1.0);
        let (size, line, pad, gap) = (14.0 * k, 19.0 * k, 10.0 * k, 14.0 * k);
        self.draw_tooltip_sized(t, size, line, pad, gap);
    }

    /// Width of the tooltip's text: the widest line, with tab columns lined up.
    fn tooltip_width(&self, t: &Tooltip, size: f32, gap: f32) -> f32 {
        let rows: Vec<Vec<&str>> = t.lines.iter().map(|(s, _)| s.split('\t').collect()).collect();
        let mut col_w: Vec<f32> = Vec::new();
        for row in rows.iter().filter(|r| r.len() > 1) {
            col_w.resize(col_w.len().max(row.len()), 0.0);
            for (k, cell) in row.iter().enumerate() {
                col_w[k] = col_w[k].max(self.measure(cell, size));
            }
        }
        let table_w = col_w.iter().sum::<f32>() + gap * col_w.len().saturating_sub(1) as f32;
        rows.iter().map(|r| if r.len() > 1 { table_w } else { self.measure(r[0], size) }).fold(0.0, f32::max)
    }

    fn draw_tooltip_sized(&self, t: &Tooltip, size: f32, line: f32, pad: f32, gap: f32) {
        let rows: Vec<Vec<&str>> = t.lines.iter().map(|(s, _)| s.split('\t').collect()).collect();
        // Columns line up across all lines that have tabs.
        let mut col_w: Vec<f32> = Vec::new();
        for row in rows.iter().filter(|r| r.len() > 1) {
            col_w.resize(col_w.len().max(row.len()), 0.0);
            for (k, cell) in row.iter().enumerate() {
                col_w[k] = col_w[k].max(self.measure(cell, size));
            }
        }
        let table_w = col_w.iter().sum::<f32>() + gap * col_w.len().saturating_sub(1) as f32;
        let line_h = |row: &[&str]| if row.iter().all(|c| c.is_empty()) { line / 2.0 } else { line };
        let content_w = rows.iter().map(|r| if r.len() > 1 { table_w } else { self.measure(r[0], size) }).fold(0.0, f32::max);
        let w = content_w + 2.0 * pad;
        let h = rows.iter().map(|r| line_h(r)).sum::<f32>() - line + size + 2.0 * pad;
        let (sw, sh) = (screen_width(), screen_height());
        let x = (t.anchor.x + t.anchor.w - w).clamp(4.0, (sw - w - 4.0).max(4.0));
        let mut y = t.anchor.y - h - 6.0;
        if y < 4.0 {
            y = (t.anchor.y + t.anchor.h + 6.0).min((sh - h - 4.0).max(4.0));
        }
        let r = Rect::new(x, y, w, h);
        draw_rectangle(r.x + 3.0, r.y + 4.0, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
        rrect(r, 5.0, BG);
        rrect_lines(r, 5.0, BORDER);
        let mut ty = r.y + pad + size * 0.8;
        for (row, (_, colour)) in rows.iter().zip(&t.lines) {
            let mut tx = r.x + pad;
            for (k, cell) in row.iter().enumerate() {
                self.text(cell, tx, ty, size, *colour);
                tx += col_w.get(k).copied().unwrap_or(0.0) + gap;
            }
            ty += line_h(row);
        }
    }

    pub fn mark(&mut self, id: &str, r: Rect) {
        self.rects.insert(id.to_string(), r);
    }

    pub fn hud(&mut self, name: &str) {
        self.hud.insert(name.to_string());
    }

    /// Hover help for `anchor`: while the mouse is over it, `lines()` (text, colour) are shown
    /// in a box above it. A tab inside a line starts a new column. The box is drawn last, on top.
    /// Registers `anchor` under `id`, and `id` as on-screen information while the box is shown.
    pub fn tooltip(&mut self, id: &str, anchor: Rect, lines: impl FnOnce() -> Vec<(String, Color)>) {
        self.mark(id, anchor);
        if self.hovered(anchor) && self.active.is_none() && self.popup.is_none() {
            self.tooltip = Some(Tooltip { anchor, lines: lines() });
            self.hud(id);
        }
    }

    /// Text of the tooltip shown this frame, lines joined by newlines (read by the self-test).
    pub fn tooltip_text(&self) -> Option<String> {
        self.tooltip.as_ref().map(|t| t.lines.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("\n"))
    }

    /// Mouse is over `r` and not covered by a popup or the top bars.
    pub fn hovered(&self, r: Rect) -> bool {
        r.contains(self.input.mouse)
            && !self.covered
            && self.input.mouse.y >= self.clip_top
            && !self.block.is_some_and(|b| b.contains(self.input.mouse))
    }

    /// Tapped (pressed and released without dragging) inside `r` this frame.
    pub fn clicked(&self, r: Rect) -> bool {
        !self.swallow && self.tap && self.hovered(r) && r.contains(self.tap_from)
    }

    /// Pressed inside `r` this frame: for things that are dragged (sliders, crop frame).
    pub fn pressed_in(&self, r: Rect) -> bool {
        !self.swallow && self.input.pressed && self.hovered(r)
    }

    /// From here on, nothing above `y` reacts to the pointer (until the next frame or reset).
    pub fn set_clip_top(&mut self, y: f32) {
        self.clip_top = y;
    }

    /// From here on, nothing reacts to the pointer, as a dialog will be drawn over it; set it
    /// back to false before drawing the dialog. Reset each frame.
    pub fn set_covered(&mut self, on: bool) {
        self.covered = on;
    }

    /// A drag that started inside `r` and is not moving a slider or a popup.
    fn dragging_in(&self, r: Rect) -> bool {
        !self.covered
            && self.dragged
            && self.input.down
            && self.active.is_none()
            && self.popup.is_none()
            && self.press_at.is_some_and(|p| r.contains(p) && p.y >= self.clip_top)
    }

    /// Scrolls a pane by pixels: mouse wheel over `r`, or a drag (touch) that began in it.
    pub fn scroll_px(&mut self, r: Rect, offset: &mut f32, max: f32) {
        if self.hovered(r) && self.input.wheel != 0.0 {
            *offset -= self.input.wheel * 40.0;
        }
        if self.dragging_in(r) {
            *offset -= self.mouse_delta.y;
        }
        *offset = offset.clamp(0.0, max.max(0.0));
    }

    /// Rows to scroll a list that moves a whole row at a time: `wheel_rows` per wheel step
    /// over `r`, or one row per `row_h` dragged. Positive moves further down the list.
    pub fn scroll_rows(&mut self, r: Rect, row_h: f32, wheel_rows: isize) -> isize {
        let mut rows = 0;
        if self.hovered(r) && self.input.wheel != 0.0 {
            rows = if self.input.wheel < 0.0 { wheel_rows } else { -wheel_rows };
        }
        if self.dragging_in(r) {
            self.row_drag -= self.mouse_delta.y;
            while self.row_drag >= row_h {
                rows += 1;
                self.row_drag -= row_h;
            }
            while self.row_drag <= -row_h {
                rows -= 1;
                self.row_drag += row_h;
            }
        }
        rows
    }

    // ---------- drawing helpers ----------

    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, color: Color) {
        draw_text_ex(
            s,
            x.round(),
            y.round(),
            TextParams { font: self.font.as_ref(), font_size: size as u16, color, ..Default::default() },
        );
    }

    pub fn measure(&self, s: &str, size: f32) -> f32 {
        measure_text(s, self.font.as_ref(), size as u16, 1.0).width
    }

    pub fn text_centered(&self, s: &str, r: Rect, size: f32, color: Color) {
        let w = self.measure(s, size);
        self.text(s, r.x + (r.w - w) / 2.0, r.y + r.h / 2.0 + size * 0.35, size, color);
    }

    /// Text cut with "..." to fit `max_w`.
    pub fn text_fit(&self, s: &str, x: f32, y: f32, size: f32, color: Color, max_w: f32) {
        if self.measure(s, size) <= max_w {
            self.text(s, x, y, size, color);
            return;
        }
        let mut t: String = s.to_string();
        while !t.is_empty() && self.measure(&format!("{t}..."), size) > max_w {
            t.pop();
        }
        self.text(&format!("{t}..."), x, y, size, color);
    }

    pub fn header(&self, s: &str, x: f32, y: f32) {
        self.text(s, x, y, 13.0, MUTED);
    }

    // ---------- widgets ----------

    pub fn button(&mut self, id: &str, r: Rect, label: &str) -> bool {
        self.button_ex(id, r, label, false)
    }

    /// "Open picture" button. In the browser the file input lies over it (see
    /// `web::picture_button`); the click then goes to the browser, not to the app.
    pub fn picture_button(&mut self, id: &str, r: Rect, label: &str) -> bool {
        #[cfg(target_arch = "wasm32")]
        if r.y >= self.clip_top && self.popup.is_none() && !self.covered {
            crate::web::picture_button(r);
        }
        self.button(id, r, label)
    }

    /// "Support on Ko-fi" button: a blue heart, then the label.
    pub fn support_button(&mut self, id: &str, r: Rect, label: &str) -> bool {
        self.mark(id, r);
        rrect(r, 5.0, if self.hovered(r) { PANEL3 } else { PANEL2 });
        let tw = self.measure(label, 14.0);
        let x = r.x + (r.w - tw - 20.0) / 2.0;
        heart(x + 6.0, r.y + r.h / 2.0, 12.0, KOFI_BLUE);
        self.text(label, x + 20.0, r.y + r.h / 2.0 + 5.0, 14.0, TEXT);
        self.clicked(r)
    }

    pub fn button_ex(&mut self, id: &str, r: Rect, label: &str, selected: bool) -> bool {
        self.mark(id, r);
        let hov = self.hovered(r);
        let bg = if selected {
            ACCENT_DIM
        } else if hov {
            PANEL3
        } else {
            PANEL2
        };
        rrect(r, 5.0, bg);
        if selected {
            rrect_lines(r, 5.0, ACCENT);
        }
        let colour = if selected { Color::new(1.0, 0.86, 0.66, 1.0) } else { TEXT };
        // a label too wide for a narrow button (phones) is set smaller, then cut
        let size = if self.measure(label, 15.0) <= r.w - 8.0 { 15.0 } else { 13.0 };
        if self.measure(label, size) <= r.w - 8.0 {
            self.text_centered(label, r, size, colour);
        } else {
            self.text_fit(label, r.x + 4.0, r.y + r.h / 2.0 + size * 0.35, size, colour, r.w - 8.0);
        }
        self.clicked(r)
    }

    /// Row of mutually exclusive options; returns the newly picked index.
    pub fn segmented(&mut self, id: &str, r: Rect, labels: &[&str], selected: usize) -> Option<usize> {
        let w = r.w / labels.len() as f32;
        let mut out = None;
        for (i, l) in labels.iter().enumerate() {
            let br = Rect::new(r.x + i as f32 * w, r.y, w - 3.0, r.h);
            if self.button_ex(&format!("{id}_{i}"), br, l, i == selected) {
                out = Some(i);
            }
        }
        out
    }

    pub fn checkbox(&mut self, id: &str, r: Rect, label: &str, value: &mut bool) -> bool {
        self.mark(id, r);
        let b = Rect::new(r.x, r.y + (r.h - 16.0) / 2.0, 16.0, 16.0);
        rrect(b, 3.0, if *value { ACCENT } else { PANEL3 });
        if *value {
            draw_line(b.x + 3.5, b.y + 8.5, b.x + 7.0, b.y + 12.0, 2.0, BG);
            draw_line(b.x + 7.0, b.y + 12.0, b.x + 13.0, b.y + 4.0, 2.0, BG);
        }
        self.text(label, r.x + 24.0, r.y + r.h / 2.0 + 5.0, 15.0, TEXT);
        if self.clicked(r) {
            *value = !*value;
            return true;
        }
        false
    }

    pub fn slider_int(&mut self, id: &str, r: Rect, value: &mut i32, min: i32, max: i32) -> bool {
        let mut f = *value as f32;
        let old = *value;
        self.slider_core(id, r, &mut f, min as f32, max as f32, (max - min) as usize);
        *value = (f.round() as i32).clamp(min, max);
        *value != old
    }

    pub fn slider_f(&mut self, id: &str, r: Rect, value: &mut f32, min: f32, max: f32) -> bool {
        let old = *value;
        self.slider_core(id, r, value, min, max, 0);
        (*value - old).abs() > f32::EPSILON
    }

    fn slider_core(&mut self, id: &str, r: Rect, value: &mut f32, min: f32, max: f32, ticks: usize) {
        self.mark(id, r);
        let track = Rect::new(r.x + 8.0, r.y + r.h / 2.0 - 2.0, r.w - 16.0, 4.0);
        if self.pressed_in(r) {
            self.active = Some(id.to_string());
        }
        if self.active.as_deref() == Some(id) && (self.input.down || self.input.pressed) {
            let t = ((self.input.mouse.x - track.x) / track.w).clamp(0.0, 1.0);
            *value = min + t * (max - min);
        }
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        rrect(track, 2.0, PANEL3);
        rrect(Rect::new(track.x, track.y, track.w * t, track.h), 2.0, ACCENT);
        if ticks > 0 && ticks <= 30 {
            for i in 0..=ticks {
                let x = track.x + track.w * i as f32 / ticks as f32;
                draw_line(x, track.y + 7.0, x, track.y + 10.0, 1.0, BORDER);
            }
        }
        let hx = track.x + track.w * t;
        let hov = self.hovered(r) || self.active.as_deref() == Some(id);
        draw_circle(hx, track.y + 2.0, if hov { 8.0 } else { 7.0 }, TEXT);
        draw_circle(hx, track.y + 2.0, 4.0, ACCENT);
    }

    pub fn text_field(&mut self, id: &str, r: Rect, value: &mut String, placeholder: &str) -> bool {
        self.mark(id, r);
        // In the browser a real (see-through) input lies over the field, so phones and
        // tablets show their keyboard; the field shows what was typed there.
        #[cfg(target_arch = "wasm32")]
        if r.y >= self.clip_top && self.popup.is_none() && !self.covered {
            let (focused, typed) = crate::web::text_field(id, r, value, placeholder);
            let mut changed = false;
            if let Some(v) = typed {
                *value = v;
                changed = true;
            }
            if focused {
                self.focus = Some(id.to_string());
            } else if self.focus.as_deref() == Some(id) {
                self.focus = None;
            }
            self.draw_field(id, r, value, placeholder);
            return changed;
        }
        let focused = self.focus.as_deref() == Some(id);
        let mut changed = false;
        if self.clicked(r) {
            self.focus = Some(id.to_string());
        } else if focused && self.input.pressed && !self.hovered(r) {
            self.focus = None;
        }
        let focused = self.focus.as_deref() == Some(id);
        if focused {
            for c in self.input.chars.clone() {
                if value.chars().count() < 60 {
                    value.push(c);
                    changed = true;
                }
            }
            if self.input.backspace && value.pop().is_some() {
                changed = true;
            }
            if self.input.enter || self.input.escape {
                self.focus = None;
            }
        }
        self.draw_field(id, r, value, placeholder);
        changed
    }

    fn draw_field(&self, id: &str, r: Rect, value: &str, placeholder: &str) {
        let focused = self.focus.as_deref() == Some(id);
        rrect(r, 4.0, if focused { BG } else { PANEL2 });
        rrect_lines(r, 4.0, if focused { ACCENT } else { BORDER });
        let ty = r.y + r.h / 2.0 + 5.0;
        if value.is_empty() && !focused {
            self.text(placeholder, r.x + 8.0, ty, 15.0, MUTED);
        } else {
            self.text_fit(value, r.x + 8.0, ty, 15.0, TEXT, r.w - 18.0);
        }
        if focused && (self.time * 2.0).fract() < 0.6 {
            let w = self.measure(value, 15.0).min(r.w - 18.0);
            draw_line(r.x + 9.0 + w, r.y + 6.0, r.x + 9.0 + w, r.y + r.h - 6.0, 1.5, ACCENT);
        }
    }

    /// Button that opens a list; returns the picked index once chosen.
    pub fn dropdown(&mut self, id: &str, r: Rect, label: &str, items: &[String]) -> Option<usize> {
        self.mark(id, r);
        let hov = self.hovered(r);
        rrect(r, 5.0, if hov { PANEL3 } else { PANEL2 });
        rrect_lines(r, 5.0, BORDER);
        self.text_fit(label, r.x + 10.0, r.y + r.h / 2.0 + 5.0, 15.0, TEXT, r.w - 34.0);
        down_arrow(r.x + r.w - 16.0, r.y + r.h / 2.0);
        match self.popup_on_click(id, r, r, || (items.to_vec(), 0)) {
            Some(Pick::Item(i)) => Some(i),
            _ => None,
        }
    }

    /// Small arrow button that opens a menu under `anchor`; returns what was done with an
    /// item. `items` (the items, and how many of the first ones can be removed) is only
    /// built when the menu opens.
    pub fn menu_button(&mut self, id: &str, r: Rect, anchor: Rect, items: impl FnOnce() -> (Vec<String>, usize)) -> Option<Pick> {
        self.mark(id, r);
        let open = self.popup.as_ref().is_some_and(|p| p.id == id);
        rrect(r, 5.0, if self.hovered(r) || open { PANEL3 } else { PANEL2 });
        down_arrow(r.x + r.w / 2.0, r.y + r.h / 2.0);
        self.popup_on_click(id, r, anchor, items)
    }

    /// Replaces the items of the open popup `id` (after one was removed), keeping it open.
    pub fn set_popup_items(&mut self, id: &str, items: Vec<String>, removable: usize) {
        if let Some(p) = self.popup.as_mut().filter(|p| p.id == id && !items.is_empty()) {
            p.items = items;
            p.removable = removable;
        }
    }

    /// Opens the popup `id` when `r` is clicked; returns what was done with an item, once.
    fn popup_on_click(&mut self, id: &str, r: Rect, anchor: Rect, items: impl FnOnce() -> (Vec<String>, usize)) -> Option<Pick> {
        if self.clicked(r) {
            let (items, removable) = items();
            if !items.is_empty() {
                let widest = items
                    .iter()
                    .map(|s| match s.split_once('\t') {
                        Some((a, b)) => self.measure(a, 15.0) + 12.0 + self.measure(b, 13.0),
                        None => self.measure(s, 15.0),
                    })
                    .fold(0.0, f32::max);
                let widest = widest + if removable > 0 { REMOVE_W - 6.0 } else { 0.0 };
                let w = (widest + 24.0).clamp(anchor.w.max(180.0), 520.0_f32.max(anchor.w)).min(screen_width() - 8.0);
                self.popup = Some(Popup { id: id.to_string(), anchor, items, removable, w });
                // Do not let this same tap close the popup again in end().
                self.swallow = true;
                self.tap = false;
            }
        }
        if let Some((pid, i)) = &self.popup_result {
            if pid == id {
                let i = *i;
                self.popup_result = None;
                return Some(i);
            }
        }
        None
    }
}

fn popup_rect(p: &Popup) -> Rect {
    let h = p.items.len() as f32 * 28.0 + 8.0;
    let y = if p.anchor.y + p.anchor.h + h > screen_height() - 4.0 {
        (p.anchor.y - h).max(4.0)
    } else {
        p.anchor.y + p.anchor.h + 2.0
    };
    let x = p.anchor.x.min(screen_width() - p.w - 4.0).max(4.0);
    Rect::new(x, y, p.w, h)
}

fn down_arrow(cx: f32, cy: f32) {
    draw_triangle(vec2(cx - 5.0, cy - 2.5), vec2(cx + 5.0, cy - 2.5), vec2(cx, cy + 3.5), MUTED);
}

pub fn rrect(r: Rect, rad: f32, c: Color) {
    let rad = rad.min(r.w / 2.0).min(r.h / 2.0);
    if rad < 0.5 {
        draw_rectangle(r.x, r.y, r.w, r.h, c);
        return;
    }
    draw_rectangle(r.x + rad, r.y, r.w - 2.0 * rad, r.h, c);
    draw_rectangle(r.x, r.y + rad, rad, r.h - 2.0 * rad, c);
    draw_rectangle(r.x + r.w - rad, r.y + rad, rad, r.h - 2.0 * rad, c);
    for (x, y) in [
        (r.x + rad, r.y + rad),
        (r.x + r.w - rad, r.y + rad),
        (r.x + rad, r.y + r.h - rad),
        (r.x + r.w - rad, r.y + r.h - rad),
    ] {
        draw_circle(x, y, rad, c);
    }
}

pub fn rrect_lines(r: Rect, rad: f32, c: Color) {
    let rad = rad.min(r.w / 2.0).min(r.h / 2.0);
    draw_line(r.x + rad, r.y + 0.5, r.x + r.w - rad, r.y + 0.5, 1.0, c);
    draw_line(r.x + rad, r.y + r.h - 0.5, r.x + r.w - rad, r.y + r.h - 0.5, 1.0, c);
    draw_line(r.x + 0.5, r.y + rad, r.x + 0.5, r.y + r.h - rad, 1.0, c);
    draw_line(r.x + r.w - 0.5, r.y + rad, r.x + r.w - 0.5, r.y + r.h - rad, 1.0, c);
    let seg = 6;
    for (cx, cy, a0) in [
        (r.x + rad, r.y + rad, std::f32::consts::PI),
        (r.x + r.w - rad, r.y + rad, 1.5 * std::f32::consts::PI),
        (r.x + r.w - rad, r.y + r.h - rad, 0.0),
        (r.x + rad, r.y + r.h - rad, 0.5 * std::f32::consts::PI),
    ] {
        for i in 0..seg {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / seg as f32;
            let b = a0 + std::f32::consts::FRAC_PI_2 * (i + 1) as f32 / seg as f32;
            let r0 = rad - 0.5;
            draw_line(cx + r0 * a.cos(), cy + r0 * a.sin(), cx + r0 * b.cos(), cy + r0 * b.sin(), 1.0, c);
        }
    }
}

pub fn col(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

/// Ko-fi's blue, for the heart on the "Support on Ko-fi" button.
pub const KOFI_BLUE: Color = Color::new(0.447, 0.643, 0.949, 1.0);

/// Small heart centred on (cx, cy), `s` px wide.
pub fn heart(cx: f32, cy: f32, s: f32, c: Color) {
    let r = s * 0.27;
    draw_circle(cx - r, cy - r * 0.4, r, c);
    draw_circle(cx + r, cy - r * 0.4, r, c);
    draw_triangle(vec2(cx - 2.0 * r, cy - r * 0.15), vec2(cx + 2.0 * r, cy - r * 0.15), vec2(cx, cy + s * 0.5), c);
}

/// Colour swatch with a thin outline.
pub fn swatch(r: Rect, c: [u8; 3]) {
    rrect(r, 4.0, col(c));
    rrect_lines(r, 4.0, Color::new(1.0, 1.0, 1.0, 0.18));
}
