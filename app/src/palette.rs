//! Paint palettes the artist builds, the pigment library and saving to disk.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Medium {
    Watercolor,
    Acrylic,
    Oil,
}

impl Medium {
    pub const ALL: [Medium; 3] = [Medium::Watercolor, Medium::Acrylic, Medium::Oil];

    pub fn label(self) -> &'static str {
        match self {
            Medium::Watercolor => "Watercolor",
            Medium::Acrylic => "Acrylic",
            Medium::Oil => "Oil",
        }
    }

    /// Watercolor lightens with water over white paper; acrylic and oil with white paint.
    pub fn uses_water(self) -> bool {
        self == Medium::Watercolor
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transparency {
    Transparent,
    HalfTransparent,
    Opaque,
}

impl Transparency {
    pub const ALL: [Transparency; 3] =
        [Transparency::Transparent, Transparency::HalfTransparent, Transparency::Opaque];

    pub fn label(self) -> &'static str {
        match self {
            Transparency::Transparent => "Transparent",
            Transparency::HalfTransparent => "Half-transparent",
            Transparency::Opaque => "Opaque",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Transparency::Transparent => "T",
            Transparency::HalfTransparent => "HT",
            Transparency::Opaque => "O",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Paint {
    pub name: String,
    pub pigment: String,
    pub transparency: Transparency,
    /// Masstone colour (sRGB) of the paint at full strength.
    pub color: [u8; 3],
    /// Tinting strength: how strongly the paint pulls a mixture (1 = average).
    #[serde(default = "one")]
    pub strength: f32,
}

fn one() -> f32 {
    1.0
}

impl Paint {
    pub fn is_white(&self) -> bool {
        let p = self.pigment.to_uppercase();
        p.starts_with("PW") && !p.contains('+')
    }

    pub fn title(&self) -> String {
        if self.pigment.trim().is_empty() {
            self.name.clone()
        } else {
            format!("{} {}", self.name, self.pigment)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Palette {
    pub name: String,
    pub medium: Medium,
    pub paints: Vec<Paint>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Store {
    pub version: u32,
    pub palettes: Vec<Palette>,
    /// The artist's own colour library (editable), shown with the fixed general `LIBRARY`.
    /// Palettes saved before it existed load with an empty one.
    #[serde(default)]
    pub library: Vec<LibColour>,
}

/// Brand shown for the fixed general library.
pub const GENERAL_BRAND: &str = "General";
/// Brand given to a new colour when nothing else suggests one.
pub const DEFAULT_BRAND: &str = "My colours";
/// Group name for colours without a brand.
pub const NO_BRAND: &str = "No brand";

/// A colour in the artist's own library: like a palette paint, plus its brand (or the
/// name of the artist's own library) and the kinds of paint it is made for.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LibColour {
    pub name: String,
    /// Pigment code(s), e.g. "PR206" or "PB29+PV19".
    pub pigment: String,
    pub transparency: Transparency,
    /// Masstone colour (sRGB).
    pub color: [u8; 3],
    #[serde(default = "one")]
    pub strength: f32,
    /// Brand or library name, e.g. "My colours".
    #[serde(default)]
    pub brand: String,
    /// Watercolor, acrylic and/or oil (at least one).
    #[serde(default = "all_media")]
    pub uses: Vec<Medium>,
}

fn all_media() -> Vec<Medium> {
    Medium::ALL.to_vec()
}

impl LibColour {
    pub fn to_paint(&self) -> Paint {
        Paint {
            name: self.name.clone(),
            pigment: self.pigment.clone(),
            transparency: self.transparency,
            color: self.color,
            strength: self.strength,
        }
    }

    pub fn from_paint(p: &Paint, brand: &str, uses: Vec<Medium>) -> Self {
        LibColour {
            name: p.name.clone(),
            pigment: p.pigment.clone(),
            transparency: p.transparency,
            color: p.color,
            strength: p.strength,
            brand: brand.to_string(),
            uses,
        }
    }

    /// A colour of the fixed general library, as a library colour for every kind of paint.
    pub fn general(lp: &LibraryPigment) -> Self {
        LibColour {
            name: lp.name.to_string(),
            pigment: lp.code.to_string(),
            transparency: lp.transparency,
            color: lp.color,
            strength: lp.strength,
            brand: GENERAL_BRAND.to_string(),
            uses: all_media(),
        }
    }

    /// Brand as shown and grouped: trimmed, "No brand" when empty.
    pub fn brand_label(&self) -> &str {
        let b = self.brand.trim();
        if b.is_empty() { NO_BRAND } else { b }
    }

    pub fn has_use(&self, m: Medium) -> bool {
        self.uses.contains(&m)
    }

    /// Group name when grouped by use: "Watercolor", "Acrylic + Oil", or "Any paint" for all three.
    pub fn uses_label(&self) -> String {
        let uses: Vec<&str> = Medium::ALL.iter().filter(|m| self.has_use(**m)).map(|m| m.label()).collect();
        match uses.len() {
            0 => "No use set".to_string(),
            3 => "Any paint".to_string(),
            _ => uses.join(" + "),
        }
    }

    /// Order of the use groups: single kinds first (watercolor, acrylic, oil), then pairs, then any.
    pub fn uses_order(&self) -> (usize, usize) {
        let first = Medium::ALL.iter().position(|m| self.has_use(*m)).unwrap_or(3);
        let n = Medium::ALL.iter().filter(|m| self.has_use(**m)).count();
        (if n == 0 { 4 } else { n }, first)
    }
}

/// Brands compare without case and surrounding spaces.
pub fn same_brand(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// A few colours of the artist's own library, for screenshots (`--my-colours`).
pub fn sample_library() -> Vec<LibColour> {
    let mk = |name: &str, code: &str, t: Transparency, color: [u8; 3], brand: &str, uses: &[Medium]| LibColour {
        name: name.into(),
        pigment: code.into(),
        transparency: t,
        color,
        strength: 1.0,
        brand: brand.into(),
        uses: uses.to_vec(),
    };
    vec![
        mk("Opera Rose", "PR122+BV10", Transparency::Transparent, [226, 70, 140], "My colours", &[Medium::Watercolor]),
        mk("Green Gold", "PY129", Transparency::Transparent, [160, 150, 40], "My colours", &[Medium::Watercolor, Medium::Acrylic]),
        mk("Shadow Violet", "PB29+PR101", Transparency::HalfTransparent, [92, 80, 118], "Studio Line", &[Medium::Watercolor]),
        mk("Warm Grey", "PW6+PBk11+PBr7", Transparency::Opaque, [150, 142, 130], "Studio Line", &[Medium::Acrylic, Medium::Oil]),
    ]
}

pub struct LibraryPigment {
    pub name: &'static str,
    pub code: &'static str,
    pub color: [u8; 3],
    pub transparency: Transparency,
    pub strength: f32,
}

use Transparency::{HalfTransparent as HT, Opaque as OP, Transparent as TR};

const fn lp(name: &'static str, code: &'static str, color: [u8; 3], transparency: Transparency, strength: f32) -> LibraryPigment {
    LibraryPigment { name, code, color, transparency, strength }
}

/// Common artist pigments with approximate masstone colours (estimated, not measured).
pub const LIBRARY: &[LibraryPigment] = &[
    lp("Hansa Yellow Light", "PY3", [246, 226, 70], TR, 1.0),
    lp("Lemon Yellow", "PY175", [240, 228, 78], TR, 0.9),
    lp("Cadmium Yellow Light", "PY35", [250, 214, 38], OP, 1.0),
    lp("Cadmium Yellow Medium", "PY35", [247, 190, 30], OP, 1.0),
    lp("Yellow Deep", "PY35", [242, 162, 24], OP, 1.0),
    lp("New Gamboge", "PY153", [232, 160, 30], HT, 1.0),
    lp("Indian Yellow", "PY139", [228, 148, 28], HT, 1.0),
    lp("Quinacridone Gold", "PO49", [190, 120, 32], TR, 1.0),
    lp("Yellow Ochre", "PY43", [200, 152, 70], OP, 0.8),
    lp("Naples Yellow", "PW6+PY35", [232, 206, 140], OP, 0.6),
    lp("Raw Sienna", "PBr7", [182, 120, 58], HT, 0.8),
    lp("Burnt Sienna", "PBr7", [142, 72, 42], TR, 1.0),
    lp("Burnt Umber", "PBr7", [90, 58, 38], HT, 1.0),
    lp("Raw Umber", "PBr7", [104, 84, 60], HT, 0.9),
    lp("Venetian Red", "PR101", [150, 58, 42], OP, 1.0),
    lp("Cadmium Orange", "PO20", [236, 104, 30], OP, 1.0),
    lp("Pyrrole Orange", "PO73", [240, 90, 33], HT, 1.2),
    lp("Cadmium Red Light", "PR108", [226, 64, 42], OP, 1.0),
    lp("Cadmium Red Medium", "PR108", [198, 44, 40], OP, 1.0),
    lp("Pyrrole Scarlet", "PR255", [218, 58, 44], HT, 1.2),
    lp("Pyrrole Red", "PR254", [204, 43, 42], HT, 1.3),
    lp("Quinacridone Rose", "PV19", [196, 48, 94], TR, 1.1),
    lp("Quinacridone Magenta", "PR122", [160, 40, 90], TR, 1.2),
    lp("Crimson", "PR206", [140, 32, 52], TR, 1.2),
    lp("Alizarin Crimson", "PR83", [138, 28, 48], TR, 1.3),
    lp("Perylene Maroon", "PR179", [108, 30, 36], TR, 1.3),
    lp("Dioxazine Violet", "PV23", [60, 32, 76], TR, 2.0),
    lp("Ultramarine Blue", "PB29", [42, 60, 156], TR, 1.0),
    lp("Cobalt Blue", "PB28", [46, 92, 170], HT, 0.7),
    lp("Cerulean Blue", "PB36", [48, 128, 176], OP, 0.7),
    lp("Phthalo Blue", "PB15:3", [14, 63, 122], TR, 2.5),
    lp("Prussian Blue", "PB27", [27, 46, 74], TR, 2.0),
    lp("Indanthrone Blue", "PB60", [31, 38, 80], TR, 1.6),
    lp("Cobalt Teal", "PG50", [59, 167, 154], HT, 0.7),
    lp("Phthalo Green", "PG7", [11, 90, 74], TR, 2.5),
    lp("Viridian", "PG18", [30, 110, 90], TR, 0.8),
    lp("Chromium Oxide Green", "PG17", [90, 122, 74], OP, 0.8),
    lp("Sap Green", "PG7+PY150", [74, 106, 42], HT, 1.0),
    lp("Payne's Grey", "PBk6+PB29", [58, 65, 80], HT, 1.2),
    lp("Neutral Tint", "PBk6+PV19+PB15", [52, 54, 58], TR, 1.4),
    lp("Ivory Black", "PBk9", [38, 36, 34], HT, 1.2),
    lp("Mars Black", "PBk11", [30, 30, 30], OP, 1.5),
    lp("Titanium White", "PW6", [246, 246, 242], OP, 1.0),
    lp("Zinc White", "PW4", [242, 242, 238], HT, 0.5),
    lp("Chinese White", "PW4", [242, 242, 238], OP, 0.5),
];

pub fn lib_paint(name: &str) -> Paint {
    let p = LIBRARY.iter().find(|p| p.name == name).expect("library pigment");
    Paint {
        name: p.name.to_string(),
        pigment: p.code.to_string(),
        transparency: p.transparency,
        color: p.color,
        strength: p.strength,
    }
}

fn custom(name: &str, code: &str, transparency: Transparency, base: &str) -> Paint {
    let mut p = lib_paint(base);
    p.name = name.to_string();
    p.pigment = code.to_string();
    p.transparency = transparency;
    p
}

/// Starter palettes, following the artist's own example for "Watercolor 1".
pub fn default_store() -> Store {
    let wc1 = Palette {
        name: "Watercolor 1".into(),
        medium: Medium::Watercolor,
        paints: vec![
            custom("Crimson", "PR206", TR, "Crimson"),
            custom("Cobalt Blue", "PB28", HT, "Cobalt Blue"),
            custom("Yellow Deep", "PY35", OP, "Yellow Deep"),
            lib_paint("Lemon Yellow"),
            lib_paint("Ultramarine Blue"),
            lib_paint("Burnt Sienna"),
            lib_paint("Phthalo Green"),
        ],
    };
    let wc2 = Palette {
        name: "Watercolor 2".into(),
        medium: Medium::Watercolor,
        paints: vec![
            lib_paint("Hansa Yellow Light"),
            lib_paint("Quinacridone Rose"),
            lib_paint("Phthalo Blue"),
            lib_paint("Raw Sienna"),
            lib_paint("Burnt Umber"),
        ],
    };
    let ac1 = Palette {
        name: "Acrylic 1".into(),
        medium: Medium::Acrylic,
        paints: vec![
            lib_paint("Titanium White"),
            lib_paint("Cadmium Yellow Medium"),
            lib_paint("Cadmium Red Medium"),
            lib_paint("Quinacridone Magenta"),
            lib_paint("Ultramarine Blue"),
            lib_paint("Phthalo Blue"),
            lib_paint("Yellow Ochre"),
            lib_paint("Burnt Umber"),
            lib_paint("Mars Black"),
        ],
    };
    let ac2 = Palette {
        name: "Acrylic 2".into(),
        medium: Medium::Acrylic,
        paints: vec![
            lib_paint("Titanium White"),
            lib_paint("Hansa Yellow Light"),
            lib_paint("Pyrrole Red"),
            lib_paint("Phthalo Blue"),
            lib_paint("Phthalo Green"),
        ],
    };
    let oil1 = Palette {
        name: "Oil 1".into(),
        medium: Medium::Oil,
        paints: vec![
            lib_paint("Titanium White"),
            lib_paint("Cadmium Yellow Light"),
            lib_paint("Yellow Ochre"),
            lib_paint("Cadmium Red Light"),
            lib_paint("Alizarin Crimson"),
            lib_paint("Ultramarine Blue"),
            lib_paint("Burnt Sienna"),
            lib_paint("Ivory Black"),
        ],
    };
    Store { version: 1, palettes: vec![wc1, wc2, ac1, ac2, oil1], library: Vec::new() }
}

/// Name of the saved palettes (see `storage`).
const STORE_FILE: &str = "palettes.json";

/// Loads saved palettes; the bool is true when nothing was saved yet (first start).
pub fn load_store() -> (Store, bool) {
    match crate::storage::read(STORE_FILE).and_then(|text| serde_json::from_str::<Store>(&text).ok()) {
        Some(store) => (store, false),
        None => (default_store(), true),
    }
}

pub fn save_store(store: &Store) -> Result<(), String> {
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    crate::storage::write(STORE_FILE, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// palettes.json saved before the colour library existed still loads, with an empty library.
    #[test]
    fn old_palettes_file_loads_without_library() {
        let old = r#"{"version":1,"palettes":[{"name":"W","medium":"Watercolor","paints":[
            {"name":"Crimson","pigment":"PR206","transparency":"Transparent","color":[140,32,52]}]}]}"#;
        let s: Store = serde_json::from_str(old).unwrap();
        assert_eq!(s.palettes[0].paints[0].name, "Crimson");
        assert!(s.library.is_empty());
    }

    /// The artist's own colours survive saving and loading; a colour saved without brand or
    /// uses gets no brand and every kind of paint.
    #[test]
    fn library_round_trips() {
        let mut s = default_store();
        s.library = sample_library();
        let text = serde_json::to_string_pretty(&s).unwrap();
        let back: Store = serde_json::from_str(&text).unwrap();
        assert_eq!(back.library, s.library);
        let bare = r#"{"name":"X","pigment":"PY3","transparency":"Opaque","color":[1,2,3]}"#;
        let c: LibColour = serde_json::from_str(bare).unwrap();
        assert_eq!(c.brand_label(), NO_BRAND);
        assert_eq!(c.uses, Medium::ALL.to_vec());
        assert_eq!(c.uses_label(), "Any paint");
    }

    #[test]
    fn brands_and_uses_group_sensibly() {
        assert!(same_brand(" My Colours ", "my colours"));
        let lib = sample_library();
        assert_eq!(lib[1].uses_label(), "Watercolor + Acrylic");
        assert!(lib[0].uses_order() < lib[1].uses_order());
        assert!(lib[1].uses_order() < LibColour::general(&LIBRARY[0]).uses_order());
    }
}
