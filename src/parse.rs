#![allow(dead_code)]

use camino::Utf8PathBuf;
use eyre::{eyre, OptionExt, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

use crate::render_opts::{MatrixPos, RenderOpts};

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
    pub matrix_lookup: MatrixLookup,
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
        let matrix_lookup = MatrixLookup::from_base_layer(base_layer);

        let combos = parse_combos_from_source(combos_def, base_layer)?;

        Ok(Self {
            layers,
            combos,
            matrix_lookup,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MatrixLookup {
    keys: HashMap<(usize, usize), Key>,
}

impl MatrixLookup {
    pub fn from_base_layer(base_layer: &Layer) -> Self {
        let keys = base_layer
            .keys
            .iter()
            .map(|key| ((key.matrix_pos.x, key.matrix_pos.y), key.clone()))
            .collect();
        Self { keys }
    }

    pub fn get(&self, col: usize, row: usize) -> Option<&Key> {
        self.keys.get(&(col, row))
    }
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
                matrix_pos: render_opts.matrix.index_to_matrix_pos(i),
            })
            .collect();

        Ok(Layer {
            id: def.layer_id,
            keys,
        })
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
    pub matrix_pos: MatrixPos,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct KeyId(pub String);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LayerId(pub String);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LayoutId(pub String);

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
        keys.sort_by_key(|k| (k.matrix_pos.x, k.matrix_pos.y));
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

        a.matrix_pos.y == b.matrix_pos.y
            && (a.matrix_pos.x as i32 - b.matrix_pos.x as i32).abs() == 1
    }

    pub fn is_vertical_neighbour(&self) -> bool {
        if self.keys.len() != 2 {
            return false;
        }
        let a = &self.keys[0];
        let b = &self.keys[1];

        a.matrix_pos.x == b.matrix_pos.x
            && (a.matrix_pos.y as i32 - b.matrix_pos.y as i32).abs() == 1
    }

    pub fn is_mid_triple(&self) -> bool {
        if self.keys.len() != 3 {
            return false;
        }
        let a = &self.keys[0];
        let b = &self.keys[1];
        let c = &self.keys[2];

        a.matrix_pos.y == b.matrix_pos.y
            && b.matrix_pos.y == c.matrix_pos.y
            && c.matrix_pos.x - b.matrix_pos.x == 1
            && b.matrix_pos.x - a.matrix_pos.x == 1
    }

    pub fn contains_input_key(&self, input: &str) -> bool {
        self.keys.iter().any(|key| key.id.0 == input)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Keyboard {}

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
    x: f32,
    y: f32,
    // TODO r
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
    use crate::render_opts::MatrixHalf;
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
  "matrix": {
    "left_rows": [5, 5, 5, 4],
    "right_rows": [5, 5, 5, 1]
  }
}
        "#;
        let render_opts = RenderOpts::parse_from_str(render_input)?;

        let keymap = Keymap::parse_from_source(keymap_c, keyboard_json, combos_def, &render_opts)?;

        assert_eq!(keymap.matrix_lookup.get(0, 0).unwrap().id.0, "SE_J");
        assert_eq!(keymap.matrix_lookup.get(2, 1).unwrap().id.0, "SE_T");
        assert_eq!(keymap.matrix_lookup.get(3, 3).unwrap().id.0, "MT_SPC");
        assert!(keymap.matrix_lookup.get(0, 4).is_none());

        assert_eq!(keymap.layers.len(), 2);
        assert_eq!(keymap.layers[0].id.0, "_BASE");
        assert_eq!(keymap.layers[0].keys.len(), 35);
        assert_eq!(keymap.layers[1].id.0, "_NUM");
        assert_eq!(keymap.layers[1].keys[1].id.0, "SE_PLUS");

        assert_eq!(keymap.combos.len(), 5);
        assert_eq!(keymap.combos[0].output, "NUMWORD");
        assert_eq!(keymap.combos[0].keys[0].id.0, "MT_SPC");
        assert_eq!(keymap.combos[0].keys[1].id.0, "SE_E");
        assert_eq!(
            keymap.combos[0].keys[0].matrix_pos,
            MatrixPos {
                x: 3,
                y: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            keymap.combos[0].keys[1].matrix_pos,
            MatrixPos {
                x: 4,
                y: 3,
                half: MatrixHalf::Right
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
