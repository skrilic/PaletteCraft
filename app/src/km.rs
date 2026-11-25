//! Kubelka-Munk paint mixing and recipe search.
//!
//! Each paint gets an absorption K and scattering S per channel. With `MixModel::Spectral`
//! (the default) the channels are `spectral::BANDS` wavelength bands of a reflectance curve
//! reconstructed from the paint's masstone colour; with `MixModel::Rgb` they are the three
//! linear RGB channels. Mixtures add K and S by concentration (two-constant Kubelka-Munk).
//! * Watercolor: the mixed film of thickness `load` sits on white paper, so
//!   adding water (lower load) lets the paper show through.
//! * Acrylic / oil: an opaque body (infinite thickness, R = R_inf(K/S)); lighter
//!   values come from adding white paint.
//!
//! Transparency sets the scattering power, tinting strength sets how much a
//! part of the paint counts in a mixture.
//!
//! With `Method::Glaze` a colour is built up in layers instead of mixed on the palette: a
//! first layer (a watercolor wash on paper; for acrylic/oil an opaque underpainting of one
//! paint and white, or the white ground itself), then up to `GLAZES_MAX` thin films of
//! single transparent or half-transparent paints, each over the dried layer below. Each film
//! is a Kubelka-Munk layer with its own reflectance R and transmittance T, stacked on the
//! layer below with R' = R + T^2 Rg / (1 - R Rg).

use crate::colour::{delta_e, lin_to_lab, lin_to_srgb8, srgb8_to_lin};
use crate::palette::{Medium, Paint, Palette, Transparency};
use crate::spectral::{self, BANDS, Basis};
use serde::{Deserialize, Serialize};

/// How mixtures are computed.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MixModel {
    /// Kubelka-Munk per wavelength band of a reconstructed reflectance curve (more accurate:
    /// e.g. complementary paints neutralise to grey, as real paints do).
    #[default]
    Spectral,
    /// Kubelka-Munk per linear RGB channel (faster, cruder).
    Rgb,
}

impl MixModel {
    pub const ALL: [MixModel; 2] = [MixModel::Spectral, MixModel::Rgb];

    pub fn label(self) -> &'static str {
        match self {
            MixModel::Spectral => "Spectral",
            MixModel::Rgb => "RGB",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            MixModel::Spectral => "spectral Kubelka-Munk (31 wavelength bands, 400-700 nm)",
            MixModel::Rgb => "Kubelka-Munk per RGB channel",
        }
    }

    /// Channels per colour: wavelength bands or RGB.
    pub fn channels(self) -> usize {
        match self {
            MixModel::Spectral => BANDS,
            MixModel::Rgb => 3,
        }
    }
}

/// How a colour is reached.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Method {
    /// Paints mixed together on the palette (and lightened with water or white).
    #[default]
    Mix,
    /// Transparent layers painted one over another once the layer below is dry.
    Glaze,
}

impl Method {
    pub const ALL: [Method; 2] = [Method::Mix, Method::Glaze];

    pub fn label(self) -> &'static str {
        match self {
            Method::Mix => "Mix",
            Method::Glaze => "Glaze",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Method::Mix => "mixing on the palette",
            Method::Glaze => "glazing: transparent layers over a first layer",
        }
    }
}

/// Glaze strengths tried: fraction of a full-strength film (watercolor: the pigment load of
/// a wash, as in `WATER_LOADS`; acrylic/oil: of the paint's full glaze thickness).
pub const GLAZE_LOADS: [f32; 6] = [1.0, 0.55, 0.3, 0.16, 0.075, 0.035];
/// Acrylic/oil: a full-strength glaze is the thinnest film of the paint that, over the white
/// ground, comes within this dE of the paint's masstone (so "strong" means the same for a
/// pale yellow as for a black).
pub const FULL_GLAZE_DE: f32 = 2.0;
/// Acrylic/oil: thickest full-strength glaze considered (K/S thickness units).
pub const FULL_GLAZE_MAX: f32 = 64.0;
/// Acrylic/oil: a full-strength glaze is also never thicker than the film that lets this
/// much light through (there and back, T^2) in the band it absorbs most; light paints such
/// as yellows approach their masstone very slowly in the bands they hardly absorb.
pub const FULL_GLAZE_T2: f32 = 0.02;
/// Most glazes over the first layer (the paints in all layers count against "Paints per mix").
pub const GLAZES_MAX: usize = 2;

/// Most channels any model uses.
pub const CH_MAX: usize = BANDS;
/// Per-channel values (only the first `MixModel::channels` are used).
pub type Ch = [f32; CH_MAX];

/// Reflectance of an sRGB colour per channel of `model`.
pub fn channels_of(model: MixModel, srgb: [u8; 3]) -> Ch {
    let lin = srgb8_to_lin(srgb);
    match model {
        MixModel::Spectral => spectral::reflectance(lin),
        MixModel::Rgb => {
            let mut c = [0.0; CH_MAX];
            c[..3].copy_from_slice(&lin);
            c
        }
    }
}

/// Watercolor paper colour (sRGB).
pub const PAPER: [u8; 3] = [247, 244, 236];
/// Default white used for acrylic/oil when the palette has none (Titanium White PW6).
pub const DEFAULT_WHITE: [u8; 3] = [246, 246, 242];
/// Water dilution steps: fraction of full pigment load.
pub const WATER_LOADS: [f32; 12] = [1.0, 0.75, 0.55, 0.4, 0.3, 0.22, 0.16, 0.11, 0.075, 0.05, 0.033, 0.02];
/// White parts tried for acrylic/oil.
pub const WHITE_PARTS: [u32; 11] = [0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 40];
/// Ratio grid: pairs use 1..=PAIR_MAX parts each, triples 1..=TRIPLE_MAX.
pub const PAIR_MAX: u32 = 6;
pub const TRIPLE_MAX: u32 = 4;
/// Score penalty (in delta E) per extra paint, so simpler mixes win ties.
pub const EXTRA_PAINT_PENALTY: f32 = 0.8;
/// Match words: a recipe below this delta E is "excellent", then "good", then "close"; above is "rough".
pub const MATCH_EXCELLENT: f32 = 2.5;
pub const MATCH_GOOD: f32 = 5.0;
pub const MATCH_CLOSE: f32 = 10.0;

pub fn scatter_for(t: Transparency) -> f32 {
    match t {
        Transparency::Transparent => 0.12,
        Transparency::HalfTransparent => 0.45,
        Transparency::Opaque => 1.6,
    }
}

#[inline]
pub fn ks_from_r(r: f32) -> f32 {
    let r = r.clamp(0.001, 0.999);
    (1.0 - r) * (1.0 - r) / (2.0 * r)
}

#[inline]
pub fn r_inf(ks: f32) -> f32 {
    1.0 + ks - (ks * ks + 2.0 * ks).sqrt()
}

/// Reflectance and transmittance of a free layer (absorption k, scattering s, thickness x).
#[inline]
pub fn film_rt(k: f32, s: f32, x: f32) -> (f32, f32) {
    let sx = s * x;
    if sx < 1e-5 {
        return (0.0, (-k * x).exp());
    }
    if k < 1e-6 {
        return (sx / (1.0 + sx), 1.0 / (1.0 + sx));
    }
    let a = 1.0 + k / s;
    let b = (a * a - 1.0).sqrt();
    let bsx = b * sx;
    if bsx > 40.0 {
        return (1.0 / (a + b), 0.0);
    }
    let (sh, ch) = (bsx.sinh(), bsx.cosh());
    let d = a * sh + b * ch;
    (sh / d, b / d)
}

/// Reflectance of a film (r, t) laid over a ground of reflectance rg.
#[inline]
pub fn over(r: f32, t: f32, rg: f32) -> f32 {
    r + t * t * rg / (1.0 - r * rg).max(1e-6)
}

/// Reflectance of a layer (absorption k, scattering s, thickness x) over a ground of reflectance rg.
#[inline]
pub fn r_layer(k: f32, s: f32, x: f32, rg: f32) -> f32 {
    let sx = s * x;
    if sx < 1e-5 {
        return rg * (-2.0 * k * x).exp();
    }
    if k < 1e-6 {
        let t = sx * (1.0 - rg);
        return (t + rg) / (1.0 + t);
    }
    let a = 1.0 + k / s;
    let b = (a * a - 1.0).sqrt();
    let bsx = b * sx;
    let coth = if bsx > 15.0 { 1.0 } else { 1.0 / bsx.tanh() };
    (1.0 - rg * (a - b * coth)) / (a - rg + b * coth)
}

#[derive(Clone, Copy, Debug)]
pub struct Optics {
    pub k: Ch,
    pub s: Ch,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lighten {
    None,
    /// Watercolor pigment load (1 = full strength).
    Water(f32),
    /// Parts of white paint.
    White(u32),
}

#[derive(Clone, Debug)]
pub struct Recipe {
    /// (palette paint index, parts). With glazes this is the first layer: one paint, or
    /// none for the bare white ground (acrylic/oil).
    pub parts: Vec<(usize, u32)>,
    pub lighten: Lighten,
    /// Glazes painted over the first layer, bottom to top: (palette paint index, strength
    /// as in `GLAZE_LOADS`). Empty for a mixture.
    pub glazes: Vec<(usize, f32)>,
    pub srgb: [u8; 3],
    pub de: f32,
}

/// A glaze film ready to stack: paint, strength and the film's R and T per channel.
struct Film {
    paint: usize,
    load: f32,
    r: Ch,
    t: Ch,
}

pub struct Mixer {
    pub medium: Medium,
    pub model: MixModel,
    /// Mix or glaze (see `with_method`).
    pub method: Method,
    /// Glaze films of the palette's transparent and half-transparent paints.
    films: Vec<Film>,
    /// Film thickness of a full-strength glaze per palette paint (watercolor: 1, the
    /// full-strength wash; acrylic/oil: see `FULL_GLAZE_DE`).
    pub glaze_full: Vec<f32>,
    /// Channels in use (`model.channels()`).
    n: usize,
    /// Reflectance per band to linear sRGB (spectral model).
    basis: Basis,
    pub optics: Vec<Optics>,
    pub names: Vec<String>,
    /// Palette indices that take part in mixtures (the white is handled separately for body paints).
    pub candidates: Vec<usize>,
    pub white: Optics,
    pub white_name: String,
    paper: Ch,
    pairs: Vec<[u32; 2]>,
    triples: Vec<[u32; 3]>,
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn solve_watercolor_k(target: f32, s: f32, rg: f32) -> f32 {
    if r_layer(0.0, s, 1.0, rg) <= target {
        return 0.0;
    }
    let (mut lo, mut hi) = (0.0f32, 80.0f32);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if r_layer(mid, s, 1.0, rg) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The glaze film of paint `paint` (optics `o`) at strength `load` of a full glaze `full` thick.
fn film_of(model: MixModel, o: &Optics, paint: usize, load: f32, full: f32) -> Film {
    let x = load * full;
    let (mut r, mut t) = ([0.0; CH_MAX], [0.0; CH_MAX]);
    for c in 0..model.channels() {
        (r[c], t[c]) = film_rt(o.k[c], o.s[c], x);
    }
    Film { paint, load, r, t }
}

pub fn optics_for(medium: Medium, model: MixModel, paint: &Paint) -> Optics {
    optics_for_colour(medium, model, paint.color, paint.transparency, paint.strength)
}

pub fn optics_for_colour(medium: Medium, model: MixModel, color: [u8; 3], transparency: Transparency, strength: f32) -> Optics {
    let refl = channels_of(model, color);
    let s0 = scatter_for(transparency);
    let mut k = [0.0; CH_MAX];
    let s = [s0; CH_MAX];
    if medium.uses_water() {
        let paper = channels_of(model, PAPER);
        for c in 0..model.channels() {
            k[c] = solve_watercolor_k(refl[c], s0, paper[c]);
        }
    } else {
        for c in 0..model.channels() {
            k[c] = s0 * ks_from_r(refl[c]);
        }
    }
    Optics { k, s, weight: strength.max(0.05) }
}

impl Mixer {
    pub fn new(palette: &Palette, model: MixModel) -> Self {
        let medium = palette.medium;
        let optics: Vec<Optics> = palette.paints.iter().map(|p| optics_for(medium, model, p)).collect();
        let names = palette.paints.iter().map(|p| p.name.clone()).collect();
        let mut white_idx = None;
        if !medium.uses_water() {
            white_idx = palette.paints.iter().position(|p| p.is_white());
        }
        let candidates = (0..palette.paints.len()).filter(|i| Some(*i) != white_idx).collect();
        let (white, white_name) = match white_idx {
            Some(i) => (optics[i], palette.paints[i].name.clone()),
            None => (
                optics_for_colour(medium, model, DEFAULT_WHITE, Transparency::Opaque, 1.0),
                "Titanium White".to_string(),
            ),
        };
        let mut pairs = Vec::new();
        for a in 1..=PAIR_MAX {
            for b in 1..=PAIR_MAX {
                if gcd(a, b) == 1 {
                    pairs.push([a, b]);
                }
            }
        }
        let mut triples = Vec::new();
        for a in 1..=TRIPLE_MAX {
            for b in 1..=TRIPLE_MAX {
                for c in 1..=TRIPLE_MAX {
                    if gcd(gcd(a, b), c) == 1 {
                        triples.push([a, b, c]);
                    }
                }
            }
        }
        let n_paints = palette.paints.len();
        let mut m = Mixer {
            medium,
            model,
            method: Method::Mix,
            films: Vec::new(),
            glaze_full: vec![1.0; n_paints],
            n: model.channels(),
            basis: spectral::basis(),
            optics,
            names,
            candidates,
            white,
            white_name,
            paper: channels_of(model, PAPER),
            pairs,
            triples,
        };
        for (i, p) in palette.paints.iter().enumerate() {
            if p.transparency == Transparency::Opaque || Some(i) == white_idx || p.is_white() {
                continue;
            }
            if !medium.uses_water() {
                m.glaze_full[i] = m.full_glaze_thickness(i);
            }
            for &load in &GLAZE_LOADS {
                let f = film_of(model, &m.optics[i], i, load, m.glaze_full[i]);
                m.films.push(f);
            }
        }
        m
    }

    /// Acrylic/oil: the thinnest film of paint `i` that, over the white ground, comes within
    /// `FULL_GLAZE_DE` of the paint's masstone or lets only `FULL_GLAZE_T2` through in its
    /// most absorbed band (at most `FULL_GLAZE_MAX`).
    fn full_glaze_thickness(&self, i: usize) -> f32 {
        let o = self.optics[i];
        let ground = self.base_reflect(&[], Lighten::None);
        let masstone = lin_to_lab(self.to_linear(&self.reflect(&o.k, &o.s, 1.0, Lighten::None)));
        let full_at = |x: f32| {
            let f = film_of(self.model, &o, i, 1.0, x);
            let de = delta_e(lin_to_lab(self.to_linear(&self.glaze_over(&f, &ground))), masstone);
            let t2_min = f.t[..self.n].iter().fold(1.0f32, |a, &t| a.min(t * t));
            de <= FULL_GLAZE_DE || t2_min <= FULL_GLAZE_T2
        };
        let de_at = |x: f32| if full_at(x) { 0.0 } else { f32::MAX };
        let mut hi = 0.05f32;
        while de_at(hi) > FULL_GLAZE_DE {
            if hi >= FULL_GLAZE_MAX {
                return FULL_GLAZE_MAX;
            }
            hi = (hi * 2.0).min(FULL_GLAZE_MAX);
        }
        let mut lo = hi / 2.0;
        for _ in 0..16 {
            let mid = 0.5 * (lo + hi);
            if de_at(mid) > FULL_GLAZE_DE {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    pub fn paper_srgb(&self) -> [u8; 3] {
        PAPER
    }

    /// Weighted K and S sums plus total weight of a set of paints.
    fn sums(&self, parts: &[(usize, u32)]) -> (Ch, Ch, f32) {
        let mut k = [0.0; CH_MAX];
        let mut s = [0.0; CH_MAX];
        let mut w = 0.0;
        for &(i, n) in parts {
            let o = &self.optics[i];
            let wi = n as f32 * o.weight;
            for c in 0..self.n {
                k[c] += wi * o.k[c];
                s[c] += wi * o.s[c];
            }
            w += wi;
        }
        (k, s, w)
    }

    /// Recipes found by `find` glaze instead of mix.
    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    /// Linear RGB of a mixture with summed K, S and weight, lightened by `lighten`.
    #[inline]
    fn finish(&self, k: &Ch, s: &Ch, w: f32, lighten: Lighten) -> [f32; 3] {
        self.to_linear(&self.reflect(k, s, w, lighten))
    }

    /// Reflectance per channel of a mixture with summed K, S and weight, lightened by `lighten`.
    #[inline]
    fn reflect(&self, k: &Ch, s: &Ch, w: f32, lighten: Lighten) -> Ch {
        let mut out = [0.0; CH_MAX];
        match lighten {
            Lighten::Water(_) | Lighten::None if self.medium.uses_water() => {
                let load = if let Lighten::Water(l) = lighten { l } else { 1.0 };
                for c in 0..self.n {
                    out[c] = r_layer(k[c] / w, s[c] / w, load, self.paper[c]);
                }
            }
            _ => {
                let wp = if let Lighten::White(n) = lighten { n as f32 * self.white.weight } else { 0.0 };
                for c in 0..self.n {
                    let kk = k[c] + wp * self.white.k[c];
                    let ss = s[c] + wp * self.white.s[c];
                    out[c] = r_inf(kk / ss.max(1e-6));
                }
            }
        }
        out
    }

    /// Reflectance of a recipe's first layer: the mixture, or with no paints the paper
    /// (watercolor) or the white ground (acrylic/oil).
    fn base_reflect(&self, parts: &[(usize, u32)], lighten: Lighten) -> Ch {
        let (k, s, w) = self.sums(parts);
        if w > 0.0 {
            return self.reflect(&k, &s, w, lighten);
        }
        if self.medium.uses_water() {
            self.paper
        } else {
            self.reflect(&self.white.k, &self.white.s, 1.0, Lighten::None)
        }
    }

    /// Reflectance after laying film `f` over a ground of reflectance `rg`.
    #[inline]
    fn glaze_over(&self, f: &Film, rg: &Ch) -> Ch {
        let mut out = [0.0; CH_MAX];
        for c in 0..self.n {
            out[c] = over(f.r[c], f.t[c], rg[c]);
        }
        out
    }

    /// Colours of a recipe layer by layer (sRGB): the first layer, then the colour after
    /// each glaze. A mixture has one stage.
    pub fn stages(&self, rc: &Recipe) -> Vec<[u8; 3]> {
        let mut r = self.base_reflect(&rc.parts, rc.lighten);
        let mut out = vec![lin_to_srgb8(self.to_linear(&r))];
        for &(p, load) in &rc.glazes {
            let f = film_of(self.model, &self.optics[p], p, load, self.glaze_full[p]);
            r = self.glaze_over(&f, &r);
            out.push(lin_to_srgb8(self.to_linear(&r)));
        }
        out
    }

    /// Whether the palette has paints transparent enough to glaze with.
    pub fn can_glaze(&self) -> bool {
        !self.films.is_empty()
    }

    /// Linear RGB of a reflectance per channel.
    #[inline]
    fn to_linear(&self, r: &Ch) -> [f32; 3] {
        match self.model {
            MixModel::Spectral => spectral::to_linear(&self.basis, r),
            MixModel::Rgb => [r[0], r[1], r[2]],
        }
    }

    /// Linear RGB reflectance of a mixture.
    pub fn mix(&self, parts: &[(usize, u32)], lighten: Lighten) -> [f32; 3] {
        let (k, s, w) = self.sums(parts);
        if w <= 0.0 {
            return match lighten {
                Lighten::White(n) if n > 0 => self.finish(&self.white.k, &self.white.s, 0.0, Lighten::White(n)),
                _ => self.to_linear(&self.paper),
            };
        }
        self.finish(&k, &s, w, lighten)
    }

    pub fn mix_srgb(&self, parts: &[(usize, u32)], lighten: Lighten) -> [u8; 3] {
        lin_to_srgb8(self.mix(parts, lighten))
    }

    /// Best recipe for a target colour (Lab), using up to `max_paints` palette paints.
    /// With `Method::Glaze` the recipe is layered (see `find_glaze`).
    pub fn find(&self, target: [f32; 3], max_paints: usize, allow_lighten: bool) -> Option<Recipe> {
        if self.candidates.is_empty() {
            return None;
        }
        if self.method == Method::Glaze {
            return self.find_glaze(target, max_paints, allow_lighten);
        }
        let lightens = self.lightens(allow_lighten);
        let mut best: Option<(f32, Recipe)> = None;
        let consider = |parts: &[(usize, u32)], best: &mut Option<(f32, Recipe)>| {
            let (k, s, w) = self.sums(parts);
            let penalty = EXTRA_PAINT_PENALTY * (parts.len() as f32 - 1.0);
            for &l in &lightens {
                let lin = self.finish(&k, &s, w, l);
                let de = delta_e(lin_to_lab(lin), target);
                let score = de + penalty;
                if best.as_ref().is_none_or(|b| score < b.0) {
                    *best = Some((
                        score,
                        Recipe { parts: parts.to_vec(), lighten: l, glazes: Vec::new(), srgb: lin_to_srgb8(lin), de },
                    ));
                }
            }
        };
        let c = &self.candidates;
        for &i in c {
            consider(&[(i, 1)], &mut best);
        }
        if max_paints >= 2 {
            for a in 0..c.len() {
                for b in a + 1..c.len() {
                    for p in &self.pairs {
                        consider(&[(c[a], p[0]), (c[b], p[1])], &mut best);
                    }
                }
            }
        }
        if max_paints >= 3 {
            for a in 0..c.len() {
                for b in a + 1..c.len() {
                    for d in b + 1..c.len() {
                        for p in &self.triples {
                            consider(&[(c[a], p[0]), (c[b], p[1]), (c[d], p[2])], &mut best);
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Lightening steps tried for a mixture or a first layer.
    fn lightens(&self, allow_lighten: bool) -> Vec<Lighten> {
        if !allow_lighten {
            vec![Lighten::None]
        } else if self.medium.uses_water() {
            WATER_LOADS.iter().map(|&l| Lighten::Water(l)).collect()
        } else {
            WHITE_PARTS.iter().map(|&n| if n == 0 { Lighten::None } else { Lighten::White(n) }).collect()
        }
    }

    /// Best layered recipe: every first layer (one paint, lightened as allowed; for
    /// acrylic/oil also the bare white ground) with up to `GLAZES_MAX` glazes of different
    /// transparent or half-transparent paints over it, at most `max_paints` paints in all.
    /// Searched exhaustively and scored like mixtures (dE plus `EXTRA_PAINT_PENALTY` per
    /// paint after the first).
    fn find_glaze(&self, target: [f32; 3], max_paints: usize, allow_lighten: bool) -> Option<Recipe> {
        let mut bases: Vec<(Option<usize>, Lighten)> = Vec::new();
        if !self.medium.uses_water() {
            bases.push((None, Lighten::None));
        }
        let lightens = self.lightens(allow_lighten);
        for &i in &self.candidates {
            for &l in &lightens {
                bases.push((Some(i), l));
            }
        }
        let max_paints = max_paints.max(1);
        let mut best: Option<(f32, Recipe)> = None;
        let mut consider = |base: (Option<usize>, Lighten), glazes: &[&Film], r: &Ch| {
            let lin = self.to_linear(r);
            let de = delta_e(lin_to_lab(lin), target);
            let paints = base.0.is_some() as usize + glazes.len();
            let score = de + EXTRA_PAINT_PENALTY * (paints as f32 - 1.0).max(0.0);
            if best.as_ref().is_none_or(|b| score < b.0) {
                best = Some((
                    score,
                    Recipe {
                        parts: base.0.map(|i| vec![(i, 1)]).unwrap_or_default(),
                        lighten: base.1,
                        glazes: glazes.iter().map(|f| (f.paint, f.load)).collect(),
                        srgb: lin_to_srgb8(lin),
                        de,
                    },
                ));
            }
        };
        for &base in &bases {
            let parts: Vec<(usize, u32)> = base.0.map(|i| vec![(i, 1)]).unwrap_or_default();
            let r0 = self.base_reflect(&parts, base.1);
            if base.0.is_some() {
                consider(base, &[], &r0);
            }
            let room = (max_paints - base.0.is_some() as usize).min(GLAZES_MAX);
            if room == 0 {
                continue;
            }
            for f1 in &self.films {
                if Some(f1.paint) == base.0 {
                    continue;
                }
                let r1 = self.glaze_over(f1, &r0);
                consider(base, &[f1], &r1);
                if room < 2 {
                    continue;
                }
                for f2 in &self.films {
                    if f2.paint == f1.paint || Some(f2.paint) == base.0 {
                        continue;
                    }
                    let r2 = self.glaze_over(f2, &r1);
                    consider(base, &[f1, f2], &r2);
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Whether a recipe is described as layers (first layer, then glazes).
    fn layered(&self, r: &Recipe) -> bool {
        !r.glazes.is_empty() || self.method == Method::Glaze
    }

    /// The paints of a recipe as pieces and the text that joins them: "3 Cobalt Blue",
    /// "1 Crimson" joined by " + " for a mixture; "Cobalt Blue", "then Crimson" joined by
    /// ", " for layers.
    pub fn part_pieces(&self, r: &Recipe) -> (Vec<String>, &'static str) {
        if self.layered(r) {
            let mut pieces = vec![match r.parts.first() {
                Some(&(i, _)) => self.names[i].clone(),
                None if self.medium.uses_water() => "Paper".to_string(),
                None => format!("{} ground", self.white_name),
            }];
            for &(i, _) in &r.glazes {
                pieces.push(format!("then {}", self.names[i]));
            }
            return (pieces, ", ");
        }
        let mut parts = r.parts.clone();
        parts.sort_by(|a, b| b.1.cmp(&a.1));
        (parts.iter().map(|&(i, n)| format!("{} {}", n, self.names[i])).collect(), " + ")
    }

    pub fn describe_parts(&self, r: &Recipe) -> String {
        let (pieces, sep) = self.part_pieces(r);
        pieces.join(sep)
    }

    /// Second line of a recipe: how much water or white (mixture), or how strong each
    /// layer is (glazing).
    pub fn describe_lighten(&self, r: &Recipe) -> String {
        if self.layered(r) {
            return self.describe_layers(r);
        }
        match r.lighten {
            Lighten::None => {
                if self.medium.uses_water() {
                    "water: full strength, very little water".into()
                } else {
                    "no white".into()
                }
            }
            Lighten::Water(load) if load >= 0.9 => "water: full strength, very little water".into(),
            Lighten::Water(load) => format!("water: {} ({:.0}% strength)", water_label(load), load * 100.0),
            Lighten::White(n) => format!("+ {} part{} {}", n, if n == 1 { "" } else { "s" }, self.white_name),
        }
    }
}

impl Mixer {
    /// How strong each layer of a layered recipe is, in the order the first line names them:
    /// watercolor "like tea / like milk / light wash"; acrylic/oil "1 paint + 3 white;
    /// glazes: thin / a veil" (the white goes into the first layer).
    fn describe_layers(&self, r: &Recipe) -> String {
        let water = self.medium.uses_water();
        let glazes: Vec<&str> = r.glazes.iter().map(|&(_, l)| if water { water_label(l) } else { glaze_label(l) }).collect();
        if water {
            let mut layers = Vec::new();
            if !r.parts.is_empty() {
                layers.push(match r.lighten {
                    Lighten::Water(l) => water_label(l),
                    _ => "full strength",
                });
            }
            layers.extend(glazes);
            return layers.join(" / ");
        }
        let base = match (r.parts.is_empty(), r.lighten) {
            (true, _) => String::new(),
            (false, Lighten::White(n)) => format!("1 paint + {n} white"),
            (false, _) => "no white".to_string(),
        };
        let g = match glazes.len() {
            0 => "no glaze".to_string(),
            1 => format!("glaze: {}", glazes[0]),
            _ => format!("glazes: {}", glazes.join(" / ")),
        };
        if base.is_empty() { g } else { format!("{base}; {g}") }
    }
}

/// Acrylic/oil glaze strength in words.
pub fn glaze_label(load: f32) -> &'static str {
    if load >= 0.9 {
        "strong"
    } else if load >= 0.45 {
        "medium"
    } else if load >= 0.25 {
        "thin"
    } else if load >= 0.12 {
        "very thin"
    } else if load >= 0.06 {
        "a veil"
    } else {
        "a faint veil"
    }
}

pub fn water_label(load: f32) -> &'static str {
    if load >= 0.9 {
        "full strength"
    } else if load >= 0.65 {
        "creamy"
    } else if load >= 0.45 {
        "like milk"
    } else if load >= 0.28 {
        "like coffee"
    } else if load >= 0.15 {
        "like tea"
    } else if load >= 0.07 {
        "light wash"
    } else {
        "very pale wash"
    }
}

pub fn match_label(de: f32) -> &'static str {
    if de < MATCH_EXCELLENT {
        "excellent"
    } else if de < MATCH_GOOD {
        "good"
    } else if de < MATCH_CLOSE {
        "close"
    } else {
        "rough"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::{hue_deg, srgb8_to_lab};
    use crate::palette::default_store;

    #[test]
    fn single_paint_reproduces_masstone() {
        for model in MixModel::ALL {
            for pal in default_store().palettes {
                let m = Mixer::new(&pal, model);
                for (i, p) in pal.paints.iter().enumerate() {
                    let got = m.mix_srgb(&[(i, 1)], Lighten::None);
                    let de = delta_e(srgb8_to_lab(got), srgb8_to_lab(p.color));
                    assert!(de < 2.0, "{model:?} {} {:?} vs {:?}", p.name, got, p.color);
                }
            }
        }
    }

    #[test]
    fn blue_and_yellow_make_green() {
        let pal = &default_store().palettes[0];
        for model in MixModel::ALL {
            let m = Mixer::new(pal, model);
            let mix = m.mix_srgb(&[(1, 1), (3, 1)], Lighten::None);
            let h = hue_deg(srgb8_to_lab(mix));
            assert!((100.0..200.0).contains(&h), "{model:?} hue {h} {:?}", mix);
        }
    }

    /// Where RGB mixing goes wrong and spectral mixing behaves like paint: complementary
    /// colours neutralise to grey, and a red with a blue leans violet, not teal.
    #[test]
    fn spectral_mixes_like_paint_where_rgb_does_not() {
        let lab_of = |pal: usize, a: usize, b: usize, model| {
            let p = &default_store().palettes[pal];
            srgb8_to_lab(Mixer::new(p, model).mix_srgb(&[(a, 1), (b, 1)], Lighten::None))
        };
        let chroma = |l: [f32; 3]| (l[1] * l[1] + l[2] * l[2]).sqrt();
        // Watercolor 1: Crimson + Phthalo Green
        let (sp, rgb) = (lab_of(0, 0, 6, MixModel::Spectral), lab_of(0, 0, 6, MixModel::Rgb));
        assert!(chroma(sp) < 6.0 && chroma(rgb) > 10.0, "crimson + phthalo green: spectral {sp:?}, RGB {rgb:?}");
        // Acrylic 2: Pyrrole Red + Phthalo Blue (a* > 0: towards red-violet)
        let (sp, rgb) = (lab_of(3, 2, 3, MixModel::Spectral), lab_of(3, 2, 3, MixModel::Rgb));
        assert!(sp[1] > 2.0 && rgb[1] < 0.0, "pyrrole red + phthalo blue: spectral {sp:?}, RGB {rgb:?}");
    }

    #[test]
    fn white_lightens_and_water_lightens() {
        for model in MixModel::ALL {
            for pal in default_store().palettes {
                let m = Mixer::new(&pal, model);
                let i = m.candidates[0];
                let base = srgb8_to_lab(m.mix_srgb(&[(i, 1)], Lighten::None))[0];
                let light = if pal.medium.uses_water() { Lighten::Water(0.1) } else { Lighten::White(8) };
                let lighter = srgb8_to_lab(m.mix_srgb(&[(i, 1)], light))[0];
                assert!(lighter > base + 5.0, "{model:?} {}: L {base} -> {lighter}", pal.name);
            }
        }
    }

    #[test]
    fn spectral_search_finds_recipes() {
        // a palette without earth colours (Acrylic 2) cannot reach an olive well in either model
        for pal in default_store().palettes {
            let target = srgb8_to_lab([120, 140, 90]);
            let sp = Mixer::new(&pal, MixModel::Spectral).find(target, 3, true).unwrap();
            let rgb = Mixer::new(&pal, MixModel::Rgb).find(target, 3, true).unwrap();
            assert!(sp.de < 6.0 || sp.de <= rgb.de, "{}: spectral dE {}, RGB dE {}", pal.name, sp.de, rgb.de);
        }
    }

    /// A film stacked with R and T over a ground equals the closed-form layer over that
    /// ground, and two films of one paint stack to one film of the summed thickness.
    #[test]
    fn films_stack_like_one_layer() {
        for (k, s) in [(0.3f32, 0.12f32), (2.0, 0.45), (0.05, 1.6), (4.0, 0.12)] {
            for rg in [0.05f32, 0.4, 0.9] {
                for (a, b) in [(0.3f32, 0.5f32), (1.0, 1.0), (0.1, 2.0)] {
                    let (r, t) = film_rt(k, s, a);
                    assert!((over(r, t, rg) - r_layer(k, s, a, rg)).abs() < 1e-4, "k {k} s {s} x {a} rg {rg}");
                    let (r2, t2) = film_rt(k, s, b);
                    let two = over(r2, t2, over(r, t, rg));
                    let one = r_layer(k, s, a + b, rg);
                    assert!((two - one).abs() < 1e-3, "k {k} s {s} {a}+{b} rg {rg}: {two} vs {one}");
                }
            }
        }
    }

    /// Watercolor: one wash in glazing is the same colour as that paint diluted on the palette.
    #[test]
    fn first_wash_matches_diluted_paint() {
        let pal = &default_store().palettes[0];
        for model in MixModel::ALL {
            let m = Mixer::new(pal, model).with_method(Method::Glaze);
            for load in [1.0, 0.3] {
                let rc = Recipe { parts: vec![(1, 1)], lighten: Lighten::Water(load), glazes: vec![], srgb: [0; 3], de: 0.0 };
                assert_eq!(m.stages(&rc)[0], m.mix_srgb(&[(1, 1)], Lighten::Water(load)));
            }
        }
    }

    /// A transparent yellow glaze over a blue wash turns it green. Its lightness barely
    /// rises (a little from the film's own scattering in the bright yellow-green band),
    /// while a glaze over white paper always darkens it.
    #[test]
    fn yellow_glaze_over_blue_is_green() {
        let pal = &default_store().palettes[0];
        let blue = pal.paints.iter().position(|p| p.name == "Cobalt Blue").unwrap();
        let yellow = pal.paints.iter().position(|p| p.name == "Lemon Yellow").unwrap();
        for model in MixModel::ALL {
            let m = Mixer::new(pal, model).with_method(Method::Glaze);
            let rc = Recipe { parts: vec![(blue, 1)], lighten: Lighten::Water(0.55), glazes: vec![(yellow, 1.0)], srgb: [0; 3], de: 0.0 };
            let st = m.stages(&rc);
            let h = hue_deg(srgb8_to_lab(st[1]));
            assert!((100.0..200.0).contains(&h), "{model:?} hue {h} {:?}", st);
            assert!(srgb8_to_lab(st[1])[0] < srgb8_to_lab(st[0])[0] + 8.0, "{model:?} glaze lightened {:?}", st);
            let on_paper = Recipe { parts: vec![], lighten: Lighten::None, glazes: vec![(yellow, 1.0)], srgb: [0; 3], de: 0.0 };
            let st = m.stages(&on_paper);
            assert!(srgb8_to_lab(st[1])[0] < srgb8_to_lab(st[0])[0], "{model:?} glaze over paper lightened {:?}", st);
        }
    }

    /// Glazing finds layered recipes in every starter palette, uses only transparent or
    /// half-transparent paints as glazes, keeps to the paint limit, and its reported colour
    /// is the colour of its layers.
    #[test]
    fn glaze_search_finds_layered_recipes() {
        let targets = [[120u8, 140, 90], [200, 120, 80], [80, 90, 130], [230, 220, 200], [60, 50, 45], [150, 60, 90]];
        for pal in default_store().palettes {
            for model in MixModel::ALL {
                let m = Mixer::new(&pal, model).with_method(Method::Glaze);
                let mut glazed = 0;
                for max in 1..=3 {
                    for t in targets {
                        let rc = m.find(srgb8_to_lab(t), max, true).unwrap();
                        assert!(rc.parts.len() + rc.glazes.len() <= max, "{}: {} paints > {max}", pal.name, rc.parts.len() + rc.glazes.len());
                        for &(g, _) in &rc.glazes {
                            assert!(pal.paints[g].transparency != Transparency::Opaque, "{}: opaque glaze {}", pal.name, pal.paints[g].name);
                        }
                        assert_eq!(*m.stages(&rc).last().unwrap(), rc.srgb);
                        assert!(!m.describe_parts(&rc).is_empty());
                        glazed += !rc.glazes.is_empty() as usize;
                    }
                }
                assert!(glazed > 0, "{} {model:?}: no recipe used a glaze", pal.name);
            }
        }
    }

    /// Acrylic/oil: for every glazing paint, a strong glaze over the white ground is strongly
    /// coloured, a veil only tints the white, and a strong glaze is no darker than the
    /// paint's masstone, whether the paint is a yellow or a black.
    #[test]
    fn glaze_strength_means_the_same_for_every_paint() {
        for pal in default_store().palettes.into_iter().filter(|p| !p.medium.uses_water()) {
            for model in MixModel::ALL {
                let m = Mixer::new(&pal, model).with_method(Method::Glaze);
                for (i, p) in pal.paints.iter().enumerate() {
                    if p.transparency == Transparency::Opaque || p.is_white() {
                        continue;
                    }
                    let at = |load: f32| {
                        let rc = Recipe { parts: vec![], lighten: Lighten::None, glazes: vec![(i, load)], srgb: [0; 3], de: 0.0 };
                        srgb8_to_lab(m.stages(&rc)[1])
                    };
                    let mass = srgb8_to_lab(m.mix_srgb(&[(i, 1)], Lighten::None));
                    let ground = at(0.0);
                    let strong = at(GLAZE_LOADS[0]);
                    let veil = at(0.075);
                    let name = &p.name;
                    assert!(delta_e(strong, ground) > 25.0, "{model:?} {name}: strong {strong:?} vs white {ground:?}");
                    assert!(delta_e(veil, ground) < 20.0, "{model:?} {name}: veil {veil:?} vs white {ground:?}");
                    assert!(strong[0] >= mass[0] - 1.0, "{model:?} {name}: strong {strong:?} darker than masstone {mass:?}");
                }
            }
        }
    }

    /// A palette with no transparent paints still gets (unglazed) recipes.
    #[test]
    fn glazing_without_transparent_paints() {
        let mut pal = default_store().palettes[2].clone();
        for p in &mut pal.paints {
            p.transparency = Transparency::Opaque;
        }
        let m = Mixer::new(&pal, MixModel::Spectral).with_method(Method::Glaze);
        assert!(!m.can_glaze());
        let rc = m.find(srgb8_to_lab([120, 140, 90]), 3, true).unwrap();
        assert!(rc.glazes.is_empty() && rc.parts.len() == 1);
    }

    #[test]
    #[ignore]
    fn explore_glazes() {
        for pal in default_store().palettes {
            let m = Mixer::new(&pal, MixModel::Spectral).with_method(Method::Glaze);
            println!("{} ({:?})", pal.name, pal.medium);
            for (i, p) in pal.paints.iter().enumerate() {
                if p.transparency == Transparency::Opaque {
                    continue;
                }
                if pal.medium.uses_water() {
                    continue;
                }
                let line: Vec<String> = GLAZE_LOADS
                    .iter()
                    .rev()
                    .map(|&l| {
                        let rc = Recipe { parts: vec![], lighten: Lighten::None, glazes: vec![(i, l)], srgb: [0; 3], de: 0.0 };
                        format!("{}:{:?}", glaze_label(l), m.stages(&rc)[1])
                    })
                    .collect();
                println!("  {:22} {:?} full {:.2}  {}", p.name, p.color, m.glaze_full[i], line.join(" "));
            }
            let t0 = std::time::Instant::now();
            for t in [[120u8, 140, 90], [200, 120, 80], [80, 90, 130], [230, 220, 200], [60, 50, 45], [150, 60, 90]] {
                let rc = m.find(srgb8_to_lab(t), 3, true).unwrap();
                let mx = Mixer::new(&pal, MixModel::Spectral).find(srgb8_to_lab(t), 3, true).unwrap();
                println!("   {t:?} glaze dE {:.1} {} | {}   (mix dE {:.1})", rc.de, m.describe_parts(&rc), m.describe_lighten(&rc), mx.de);
            }
            println!("   {:.1} ms per glaze search", t0.elapsed().as_secs_f64() * 1000.0 / 6.0);
        }
    }

    #[test]
    #[ignore]
    fn explore() {
        for pal in default_store().palettes {
            for model in MixModel::ALL {
                let m = Mixer::new(&pal, model);
                let mut line = format!("{:12} {:8}", pal.name, model.label());
                let n = pal.paints.len();
                for i in 0..n {
                    for j in i + 1..n {
                        let c = m.mix_srgb(&[(i, 1), (j, 1)], Lighten::None);
                        let lab = srgb8_to_lab(c);
                        line += &format!(" {i}+{j}:{c:?}C{:.0}", (lab[1] * lab[1] + lab[2] * lab[2]).sqrt());
                    }
                }
                println!("{line}");
                for t in [[120u8, 140, 90], [200, 120, 80], [80, 90, 130], [230, 220, 200], [60, 50, 45]] {
                    let rc = m.find(srgb8_to_lab(t), 3, true).unwrap();
                    println!("   target {t:?} -> {:?} dE {:.1} {} {}", rc.srgb, rc.de, m.describe_parts(&rc), m.describe_lighten(&rc));
                }
            }
        }
    }
}
