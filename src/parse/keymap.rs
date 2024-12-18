use crate::parse::Finger;
use crate::parse::FingerAssignment;
use camino::Utf8PathBuf;
use eyre::{eyre, OptionExt, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::sync::LazyLock;

use super::render_opts::{PhysicalPos, RenderOpts};

#[derive(Debug)]
pub struct ParseSettings {
    pub qmk_root: Utf8PathBuf,
    pub keyboard: String,
    pub keymap: String,
}

impl ParseSettings {
    pub fn combos_def(&self) -> Utf8PathBuf {
        self.keymap_dir().join("combos.def")
    }

    pub fn keymap_c(&self) -> Utf8PathBuf {
        self.keymap_dir().join("keymap.c")
    }

    pub fn keyboard_json(&self) -> Utf8PathBuf {
        self.keyboard_dir().join("keyboard.json")
    }

    pub fn info_json(&self) -> Utf8PathBuf {
        self.keyboard_dir().join("info.json")
    }

    pub fn keyboard_dir(&self) -> Utf8PathBuf {
        self.qmk_root.join("keyboards").join(&self.keyboard)
    }

    pub fn base_keyboard_dir(&self) -> Utf8PathBuf {
        let part = if let Some((part, _)) = self.keyboard.split_once("/") {
            part
        } else {
            self.keyboard.as_str()
        };
        self.qmk_root.join("keyboards").join(part)
    }

    pub fn keymap_dir(&self) -> Utf8PathBuf {
        self.base_keyboard_dir().join("keymaps").join(&self.keymap)
    }
}

#[derive(Debug, Clone)]
pub struct Keymap {
    pub layers: Vec<Layer>,
    pub combos: Vec<Combo>,
}

impl Keymap {
    pub fn parse(input: &ParseSettings, render_opts: &RenderOpts) -> Result<Self> {
        let keymap_c = fs::read_to_string(input.keymap_c())?;
        let keyboard_json_path = input.keyboard_json();
        let info_json_path = input.info_json();
        let info = if keyboard_json_path.is_file() {
            fs::read_to_string(keyboard_json_path)?
        } else if info_json_path.is_file() {
            fs::read_to_string(info_json_path)?
        } else {
            return Err(eyre!("Couldn't find keyboard.json or info.json at {keyboard_json_path} nor {info_json_path}"));
        };

        let combos_def = fs::read_to_string(input.combos_def())?;
        Self::parse_from_source(&keymap_c, &info, &combos_def, render_opts)
    }

    pub fn parse_from_source(
        keymap_c: &str,
        keyboard_json: &str,
        combos_def: &str,
        render_opts: &RenderOpts,
    ) -> Result<Self> {
        let layer_defs = parse_layers_from_source(keymap_c)?;
        let keyboard_spec: KeyboardSpec = serde_json::from_str(keyboard_json)?;

        let layers = layer_defs
            .into_iter()
            .map(|def| Layer::new(def, &keyboard_spec, render_opts))
            .collect::<Result<Vec<_>>>()?;

        let base_layer = &layers[0];

        let combos = parse_combos_from_source(combos_def, base_layer)?;

        Ok(Self { layers, combos })
    }

    pub fn get_layer_id(&self, i: usize) -> Option<LayerId> {
        self.layers.get(i).map(|layer| layer.id.clone())
    }

    pub fn find_key_by_matrix(&self, highest_layer: usize, pos: (usize, usize)) -> Option<&Key> {
        let mut curr_layer = highest_layer;
        loop {
            let layer = &self.layers[curr_layer];

            if let Some(key) = layer.find_key_by_matrix(pos) {
                if !is_fallback_key(&key.id) {
                    return Some(key);
                }
            }

            if curr_layer == 0 {
                return None;
            }
            curr_layer -= 1;
        }
    }
}

fn is_fallback_key(id: &KeyId) -> bool {
    matches!(id.0.as_str(), "_______" | "xxxxxxx")
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub keys: Vec<Key>,
}

impl Layer {
    pub fn new(def: LayerDef, spec: &KeyboardSpec, render_opts: &RenderOpts) -> Result<Self> {
        let layout_id = &def.layout_id.0;
        let layout_spec = spec
            .get_layout(layout_id)
            .ok_or_eyre(format!("Failed to find layout spec for {}", layout_id))?;

        if def.keys.len() != layout_spec.layout.len() {
            return Err(eyre!(
                "Layer and it's spec has a mismatched number of keys {} != {} for layer {}",
                def.keys.len(),
                layout_spec.layout.len(),
                def.layer_id.0
            ));
        }

        let keys = def
            .keys
            .into_iter()
            .zip(layout_spec.layout.iter())
            .enumerate()
            .map(|(i, (id, spec))| Key {
                id,
                x: spec.x,
                y: spec.y,
                matrix_pos: spec.matrix,
                physical_pos: render_opts.physical_layout.index_to_pos(i),
            })
            .collect();

        Ok(Layer {
            id: def.layer_id,
            keys,
        })
    }

    pub fn find_key_by_id(&self, id: &str) -> Option<&Key> {
        self.keys.iter().find(|key| key.id.0 == id)
    }

    pub fn find_key_by_matrix(&self, pos: (usize, usize)) -> Option<&Key> {
        self.keys.iter().find(|key| key.matrix_pos == pos)
    }

    pub fn find_key_by_physical_pos(&self, pos: (usize, usize)) -> Option<&Key> {
        self.keys.iter().find(|key| key.physical_pos.pos() == pos)
    }

    pub fn replace_key_id(&mut self, key_id: &str, replacement: &str) {
        if let Some(key) = self.keys.iter_mut().find(|key| key.id.0 == key_id) {
            key.id = KeyId(replacement.to_owned())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Key {
    pub id: KeyId,
    pub x: f32,
    pub y: f32,
    pub physical_pos: PhysicalPos,
    pub matrix_pos: (usize, usize),
}

impl Key {
    pub fn is_sfb(&self, other: &Key) -> bool {
        self.physical_pos.is_sfb(&other.physical_pos)
    }
}

impl Eq for Key {}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id) && self.matrix_pos == other.matrix_pos
    }
}

impl Hash for Key {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.0.hash(state);
        self.matrix_pos.hash(state);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct KeyId(pub String);

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LayerId(pub String);

impl std::fmt::Display for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LayoutId(pub String);

impl std::fmt::Display for LayoutId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LayerDef {
    pub layer_id: LayerId,
    pub layout_id: LayoutId,
    pub keys: Vec<KeyId>,
}

#[derive(Debug, Clone)]
pub struct Combo {
    pub id: String,
    pub output: String,
    pub keys: Vec<Key>,
}

impl Combo {
    pub fn new(id: String, output: String, mut keys: Vec<Key>) -> Self {
        // Make sure that keys are sorted in matrix position
        keys.sort_by_key(|k| (k.physical_pos.col, k.physical_pos.row));
        Combo { id, output, keys }
    }

    pub fn min_x(&self) -> f32 {
        self.keys
            .iter()
            .min_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
            .unwrap()
            .x
    }

    pub fn max_x(&self) -> f32 {
        self.keys
            .iter()
            .max_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
            .unwrap()
            .x
    }

    pub fn min_y(&self) -> f32 {
        self.keys
            .iter()
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap()
            .y
    }

    pub fn max_y(&self) -> f32 {
        self.keys
            .iter()
            .max_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap()
            .y
    }

    pub fn is_horizontal_neighbour(&self) -> bool {
        if self.keys.len() != 2 {
            return false;
        }
        let a = &self.keys[0];
        let b = &self.keys[1];

        a.physical_pos.row == b.physical_pos.row
            && (a.physical_pos.col as i32 - b.physical_pos.col as i32).abs() == 1
    }

    pub fn is_vertical_neighbour(&self) -> bool {
        if self.keys.len() != 2 {
            return false;
        }
        let a = &self.keys[0];
        let b = &self.keys[1];

        a.physical_pos.col == b.physical_pos.col
            && (a.physical_pos.row as i32 - b.physical_pos.row as i32).abs() == 1
    }

    pub fn is_mid_triple(&self) -> bool {
        if self.keys.len() != 3 {
            return false;
        }
        let a = &self.keys[0];
        let b = &self.keys[1];
        let c = &self.keys[2];

        a.physical_pos.row == b.physical_pos.row
            && b.physical_pos.row == c.physical_pos.row
            && c.physical_pos.col - b.physical_pos.col == 1
            && b.physical_pos.col - a.physical_pos.col == 1
    }

    pub fn contains_input_key(&self, input: &str) -> bool {
        self.keys.iter().any(|key| key.id.0 == input)
    }

    pub fn contains_physical_pos(&self, pos: (usize, usize)) -> bool {
        self.keys
            .iter()
            .any(|combo_key| combo_key.physical_pos.pos() == pos)
    }

    pub fn contains_finger(&self, finger: &FingerAssignment) -> bool {
        self.keys
            .iter()
            .any(|combo_key| combo_key.physical_pos.finger == *finger)
    }

    pub fn is_key_sfb(&self, key: &Key) -> bool {
        // Can't have the same position in the combo
        if self.contains_physical_pos(key.physical_pos.pos()) {
            return false;
        }

        self.contains_finger(&key.physical_pos.finger)
    }

    pub fn get_fingers(&self) -> HashSet<FingerAssignment> {
        self.keys
            .iter()
            .map(|key| key.physical_pos.finger)
            .collect()
    }

    pub fn get_positions(&self) -> HashSet<(usize, usize)> {
        self.keys.iter().map(|key| key.physical_pos.pos()).collect()
    }

    pub fn is_combo_sfb(&self, combo: &Combo) -> bool {
        if self
            .get_positions()
            .intersection(&combo.get_positions())
            .next()
            .is_some()
        {
            return false;
        }

        self.get_fingers()
            .intersection(&combo.get_fingers())
            .next()
            .is_some()
    }
}

#[derive(Deserialize, Debug)]
pub struct KeyboardSpec {
    layouts: HashMap<String, LayoutSpec>,
    layout_aliases: Option<HashMap<String, String>>,
}

impl KeyboardSpec {
    pub fn get_layout(&self, id: &str) -> Option<&LayoutSpec> {
        if let Some(layout) = self.layouts.get(id) {
            return Some(layout);
        }

        if let Some(alias) = self.layout_aliases.as_ref().and_then(|map| map.get(id)) {
            return self.get_layout(alias);
        }
        None
    }
}

#[derive(Deserialize, Debug)]
pub struct LayoutSpec {
    layout: Vec<KeySpec>,
}

#[derive(Deserialize, Debug)]
pub struct KeySpec {
    matrix: (usize, usize),
    x: f32,
    y: f32,
}

fn parse_layers_from_source(src: &str) -> Result<Vec<LayerDef>> {
    static KEYMAPS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?msx)const\s+uint16_t\s+PROGMEM\s+keymaps\[\]\[\w+\]\[\w+\]\s*=\s*\{(.+)};")
            .unwrap()
    });
    static LAYER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?msx)
        \[([\w\d_]+)\]\s*=\s*([\w\d_]+)\(
            (.+?)
        ^\s*\),?$",
        )
        .unwrap()
    });

    if let Some(keymaps) = KEYMAPS.captures(src) {
        let layers_str = keymaps[1].trim();
        let layers = LAYER
            .captures_iter(layers_str)
            .map(|caps| {
                let layer_id = LayerId(caps[1].to_string());
                let layout_id = LayoutId(caps[2].to_string());
                let keys: Vec<_> = caps[3]
                    .split(",")
                    .map(|x| KeyId(x.trim().to_string()))
                    .collect();
                LayerDef {
                    layer_id,
                    layout_id,
                    keys,
                }
            })
            .collect();
        Ok(layers)
    } else {
        Ok(Vec::new())
    }
}

fn parse_combos_from_source(src: &str, base_layer: &Layer) -> Result<Vec<Combo>> {
    let key_lookup: HashMap<String, Key> = base_layer
        .keys
        .iter()
        .map(|key| (key.id.0.to_owned(), key.clone()))
        .collect();

    static SPEC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*(COMB|SUBS)\((.+)\)\s*$").unwrap());
    static QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^"([^"]+)"$"#).unwrap());

    let mut res = Vec::new();
    for line in src.lines() {
        if let Some(spec) = SPEC.captures(line) {
            let args: Vec<_> = spec[2].split(",").map(|x| x.trim()).collect();
            let id = args[0].to_string();
            let output_s = args[1].to_string();
            let output = match &spec[1] {
                "SUBS" => match QUOTES.captures(&output_s) {
                    Some(x) => x[1].to_string(),
                    None => output_s,
                },
                "COMB" => output_s,
                _ => panic!("No SUBS or COMB in regex match {}", &spec[1]),
            };

            let keys = args[2..]
                .iter()
                .map(|x| {
                    key_lookup
                        .get(*x)
                        .cloned()
                        .ok_or_eyre(format!("Couldn't find combo `{x}` in base layer"))
                })
                .collect::<Result<Vec<_>>>()?;
            res.push(Combo::new(id, output, keys));
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::MatrixHalf;
    use eyre::Result;

    #[test]
    fn test_parse_keymap() -> Result<()> {
        let keymap_c = r#"
// clang-format off
const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [_BASE] = LAYOUT(
      SE_J,    SE_C,    SE_Y,    SE_F,    SE_P,         SE_X,    SE_W,    SE_O,    SE_U,    SE_DOT,
      SE_R,    SE_S,    SE_T,    SE_H,    SE_K,         SE_M,    SE_N,    SE_A,    SE_I,    REPEAT,
      SE_COMM, SE_V,    SE_G,    SE_D,    SE_B,         SE_SLSH, SE_L,    SE_LPRN, SE_RPRN, SE_UNDS,
               xxxxxxx, xxxxxxx,
                                 FUN,     MT_SPC,       SE_E
    ),
    [_NUM]  = LAYOUT(
      SE_J,    SE_PLUS, SE_ASTR, SE_EXLM, SE_P,         SE_X,    _______, AT_U,    REPEAT,  _______,
      SE_6,    SE_4,    SE_0,    SE_2,    SE_K,         _______, SE_3,    SE_1,    SE_5,    SE_7,
      SE_COMM, _______, NUM_G,   SE_8,    _______,      SE_SLSH, SE_9,    SE_LPRN, SE_RPRN, SE_UNDS,
               _______, _______,
                                 _______, _______,      _______
    )
};
        "#;
        let keyboard_json = r#"
{
    "layouts": {
        "LAYOUT": {
            "layout": [
                { "matrix": [1, 0], "x": 0, "y": 0.93 },
                { "matrix": [0, 1], "x": 1, "y": 0.31 },
                { "matrix": [0, 2], "x": 2, "y": 0 },
                { "matrix": [0, 3], "x": 3, "y": 0.28 },
                { "matrix": [0, 4], "x": 4, "y": 0.42 },
                { "matrix": [4, 0], "x": 7, "y": 0.42 },
                { "matrix": [4, 1], "x": 8, "y": 0.28 },
                { "matrix": [4, 2], "x": 9, "y": 0 },
                { "matrix": [4, 3], "x": 10, "y": 0.31 },
                { "matrix": [4, 4], "x": 11, "y": 0.93 },

                { "matrix": [2, 0], "x": 0, "y": 1.93 },
                { "matrix": [1, 1], "x": 1, "y": 1.31 },
                { "matrix": [1, 2], "x": 2, "y": 1 },
                { "matrix": [1, 3], "x": 3, "y": 1.28 },
                { "matrix": [1, 4], "x": 4, "y": 1.42 },
                { "matrix": [5, 0], "x": 7, "y": 1.42 },
                { "matrix": [5, 1], "x": 8, "y": 1.28 },
                { "matrix": [5, 2], "x": 9, "y": 1 },
                { "matrix": [5, 3], "x": 10, "y": 1.31 },
                { "matrix": [5, 4], "x": 11, "y": 1.93 },

                { "matrix": [3, 0], "x": 0, "y": 2.93 },
                { "matrix": [2, 1], "x": 1, "y": 2.31 },
                { "matrix": [2, 2], "x": 2, "y": 2 },
                { "matrix": [2, 3], "x": 3, "y": 2.28 },
                { "matrix": [2, 4], "x": 4, "y": 2.42 },
                { "matrix": [6, 0], "x": 7, "y": 2.42 },
                { "matrix": [6, 1], "x": 8, "y": 2.28 },
                { "matrix": [6, 2], "x": 9, "y": 2 },
                { "matrix": [6, 3], "x": 10, "y": 2.31 },
                { "matrix": [6, 4], "x": 11, "y": 2.93 },

                { "matrix": [3, 1], "x": 1, "y": 3.31 },
                { "matrix": [3, 2], "x": 2, "y": 3 },

                { "matrix": [3, 3], "x": 3.5, "y": 3.75 },
                { "matrix": [3, 4], "x": 4.5, "y": 4 },
                { "matrix": [7, 0], "x": 6.5, "y": 4 }
            ]
        }
    }
}
        "#;

        let combos_def = r##"
// Comment
COMB(num,               NUMWORD,        MT_SPC, SE_E)

SUBS(https,             "https://",     MT_SPC, SE_SLSH)
COMB(comb_boot_r,       QK_BOOT,        SE_E, SE_L, SE_LPRN, SE_RPRN, SE_UNDS)

COMB(escape_sym,        ESC_SYM,        SE_T, SE_H)
SUBS(lt_eq,             "<=",           SE_F, SE_H)

SUBS(el_str_int,        "#{}"SS_TAP(X_LEFT),  SE_X, SE_W)
        "##;

        let render_input = r#"
{
  "layers": {},
  "colors": {},
  "legend": [],
  "outputs": {
    "combo_keys_with_separate_imgs": [],
    "combo_highlight_groups": {},
    "combo_background_layer_class": "combo_background",
    "active_class_in_separate_layer": "active_layer"
  },
  "physical_layout": [
    "54446    64445",
    "21005    50012",
    "64436    63446",
    " 77",
    "   80    0"
  ],
  "finger_assignments": [
    "11233    33211",
    "01233    33210",
    "01233    33210",
    " 12",
    "   44    4"
  ]
}
        "#;
        let render_opts = RenderOpts::parse_from_str("id", render_input)?;

        let keymap = Keymap::parse_from_source(keymap_c, keyboard_json, combos_def, &render_opts)?;

        assert_eq!(keymap.layers.len(), 2);
        assert_eq!(keymap.layers[0].id.0, "_BASE");
        assert_eq!(keymap.layers[0].keys.len(), 35);
        assert_eq!(keymap.layers[1].id.0, "_NUM");
        assert_eq!(keymap.layers[1].keys[1].id.0, "SE_PLUS");

        let base = &keymap.layers[0];
        assert_eq!(base.find_key_by_matrix((1, 0)).unwrap().id.0, "SE_J");
        assert_eq!(base.find_key_by_matrix((0, 1)).unwrap().id.0, "SE_C");
        assert_eq!(base.find_key_by_matrix((6, 4)).unwrap().id.0, "SE_UNDS");
        assert_eq!(base.find_key_by_matrix((3, 4)).unwrap().id.0, "MT_SPC");
        assert_eq!(base.find_key_by_physical_pos((0, 0)).unwrap().id.0, "SE_J");
        assert_eq!(base.find_key_by_physical_pos((1, 0)).unwrap().id.0, "SE_C");
        assert_eq!(base.find_key_by_physical_pos((2, 0)).unwrap().id.0, "SE_Y");
        assert_eq!(base.find_key_by_physical_pos((3, 0)).unwrap().id.0, "SE_F");
        assert_eq!(base.find_key_by_physical_pos((4, 0)).unwrap().id.0, "SE_P");
        assert_eq!(base.find_key_by_physical_pos((5, 0)).unwrap().id.0, "SE_X");
        assert_eq!(base.find_key_by_physical_pos((6, 0)).unwrap().id.0, "SE_W");
        assert_eq!(base.find_key_by_physical_pos((7, 0)).unwrap().id.0, "SE_O");
        assert_eq!(base.find_key_by_physical_pos((8, 0)).unwrap().id.0, "SE_U");
        assert_eq!(
            base.find_key_by_physical_pos((9, 0)).unwrap().id.0,
            "SE_DOT"
        );
        assert_eq!(base.find_key_by_physical_pos((0, 1)).unwrap().id.0, "SE_R");

        assert_eq!(keymap.combos.len(), 6);
        assert_eq!(keymap.combos[0].output, "NUMWORD");
        assert_eq!(keymap.combos[0].keys[0].id.0, "MT_SPC");
        assert_eq!(keymap.combos[0].keys[1].id.0, "SE_E");
        assert_eq!(
            keymap.combos[0].keys[0].physical_pos,
            PhysicalPos {
                col: 4,
                row: 4,
                effort: 0,
                finger: FingerAssignment {
                    finger: Finger::Thumb,
                    half: MatrixHalf::Left
                }
            }
        );
        assert_eq!(
            keymap.combos[0].keys[1].physical_pos,
            PhysicalPos {
                col: 5,
                row: 4,
                effort: 0,
                finger: FingerAssignment {
                    finger: Finger::Thumb,
                    half: MatrixHalf::Right
                }
            }
        );
        assert!(keymap.combos[1].contains_input_key("MT_SPC"));
        assert!(!keymap.combos[3].contains_input_key("MT_SPC"));
        assert!(keymap.combos[3].is_horizontal_neighbour());
        assert!(!keymap.combos[3].is_vertical_neighbour());
        assert!(!keymap.combos[4].is_horizontal_neighbour());
        assert!(keymap.combos[4].is_vertical_neighbour());

        assert_eq!(keymap.combos[5].output, "\"#{}\"SS_TAP(X_LEFT)");

        Ok(())
    }
}
