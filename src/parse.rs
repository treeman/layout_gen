use eyre::{eyre, OptionExt, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

use crate::InputSettings;

#[derive(Debug, Clone)]
pub struct Keymap {
    pub layers: Vec<Layer>,
    pub combos: Vec<Combo>,
}

impl Keymap {
    pub fn parse(input: &InputSettings) -> Result<Self> {
        let keymap_c = fs::read_to_string(input.keymap_c())?;
        let layer_defs = parse_layers_from_source(&keymap_c)?;

        let combos_def = fs::read_to_string(input.combos_def())?;
        let combos = parse_combos_from_source(&combos_def)?;

        let keyboard_json = fs::read_to_string(input.keyboard_json())?;
        let keyboard_spec: KeyboardSpec = serde_json::from_str(&keyboard_json)?;

        let layers = layer_defs
            .into_iter()
            .map(|def| Layer::new(def, &keyboard_spec))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { layers, combos })
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub keys: Vec<Key>,
}

impl Layer {
    pub fn new(def: LayerDef, spec: &KeyboardSpec) -> Result<Self> {
        let layout_id = &def.layout_id.0;
        let layout_spec = spec
            .layouts
            .get(layout_id)
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
            .map(|(id, spec)| Key {
                id,
                x: spec.x,
                y: spec.y,
            })
            .collect();

        Ok(Layer {
            id: def.layer_id,
            keys,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Key {
    pub id: KeyId,
    pub x: f32,
    pub y: f32,
    // TODO rotation
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ComboOutput {
    Key(KeyId),
    String(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Combo {
    pub output: ComboOutput,
    pub keys: Vec<KeyId>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Keyboard {}

#[derive(Deserialize, Debug)]
pub struct KeyboardSpec {
    layouts: HashMap<String, LayoutSpec>,
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

fn parse_combos_from_source(src: &str) -> Result<Vec<Combo>> {
    static SPEC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*(COMB|SUBS)\((.+)\)\s*$").unwrap());

    let mut res = Vec::new();
    for line in src.lines() {
        if let Some(spec) = SPEC.captures(line) {
            let args: Vec<_> = spec[2].split(",").map(|x| x.trim()).collect();
            let output_s = args[1].to_string();
            let output = match &spec[1] {
                "SUBS" => ComboOutput::String(output_s[1..output_s.len() - 1].to_string()),
                "COMB" => ComboOutput::Key(KeyId(output_s)),
                _ => panic!("No SUBS or COMB in regex match {}", &spec[1]),
            };

            let keys = args[2..].iter().map(|x| KeyId(x.to_string())).collect();
            res.push(Combo { output, keys });
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::Result;

    #[test]
    fn test_parse_layers() -> Result<()> {
        let input = r#"
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
        let layers = parse_layers_from_source(input)?;
        assert!(layers.len() == 2);
        assert!(layers[0].layer_id.0 == "_BASE");
        assert!(layers[0].keys.len() == 35);
        assert!(layers[1].layer_id.0 == "_NUM");
        assert!(layers[1].keys[1].0 == "SE_PLUS");
        Ok(())
    }

    #[test]
    fn test_parse_combos() -> Result<()> {
        let input = r#"
// Thumbs
COMB(num,               NUMWORD,        MT_SPC, SE_E)

SUBS(https,             "https://",     MT_SPC, SE_SLSH)
COMB(comb_boot_r,       QK_BOOT,        SE_E, SE_L, SE_LPRN, SE_RPRN, SE_UNDS)
        "#;
        let combos = parse_combos_from_source(input)?;
        assert!(combos.len() == 3);
        assert!(combos[0].output == ComboOutput::Key(KeyId("NUMWORD".to_string())));
        assert!(combos[0].keys == vec![KeyId("MT_SPC".to_string()), KeyId("SE_E".to_string())]);
        assert!(combos[1].output == ComboOutput::String("https://".to_string()));
        assert!(combos[1].keys == vec![KeyId("MT_SPC".to_string()), KeyId("SE_SLSH".to_string())]);
        Ok(())
    }

    #[test]
    fn test_parse_keyboard() -> Result<()> {
        let input = r#"
{
    "layouts": {
        "LAYOUT": {
            "layout": [
                { "matrix": [1, 0], "x": 0, "y": 1 },
                { "matrix": [0, 1], "x": 2, "y": 3 }
            ]
        }
    }
}
        "#;

        let _spec: KeyboardSpec = serde_json::from_str(input)?;
        Ok(())
    }
}
