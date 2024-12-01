use camino::Utf8Path;
use eyre::Result;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct RenderOpts {
    pub id: String,
    pub default_keys: HashMap<String, PartialKeyOpts>,
    pub layer_keys: HashMap<String, HashMap<String, PartialKeyOpts>>,
    pub legend: Vec<LegendSpec>,
    pub colors: HashMap<String, String>,
    pub matrix: MatrixSpec,
    pub combos: CombosSpec,
}

impl RenderOpts {
    pub fn parse(file: &Utf8Path) -> Result<Self> {
        let src = fs::read_to_string(file)?;
        Self::parse_from_str(file.file_stem().unwrap(), &src)
    }

    pub fn parse_from_str(id: &str, s: &str) -> Result<Self> {
        let spec: RenderSpec = serde_json::from_str(s)?;
        Ok(Self::new(id, spec))
    }

    fn new(id: &str, spec: RenderSpec) -> Self {
        let mut default_keys = HashMap::new();
        let mut layer_keys: HashMap<String, HashMap<String, PartialKeyOpts>> = HashMap::new();

        for (layer_id, layer) in spec.layers {
            for key_spec in &layer {
                for key in &key_spec.keys {
                    let opts = PartialKeyOpts::from_spec(key, key_spec);

                    if layer_id == "default" {
                        default_keys.insert(key.to_owned(), opts);
                    } else {
                        layer_keys
                            .entry(layer_id.to_string())
                            .or_default()
                            .insert(key.to_owned(), opts);
                    }
                }
            }
        }
        Self {
            id: id.into(),
            default_keys,
            layer_keys,
            legend: spec.legend,
            colors: spec.colors,
            matrix: spec.matrix,
            combos: spec.combos,
        }
    }

    pub fn get(&self, layer_id: &str, key_id: &str) -> KeyOpts {
        let mut res = KeyOpts::with_defaults(key_id);

        if let Some(opts) = self.default_keys.get(key_id) {
            res.merge(opts);
        }

        if let Some(keys) = self.layer_keys.get(layer_id) {
            if let Some(opts) = keys.get(key_id) {
                res.merge(opts);
            }
        }
        res
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyOpts {
    pub id: String,
    pub title: String,
    pub hold_title: Option<String>,
    pub class: String,
}

impl KeyOpts {
    fn with_defaults(key_id: &str) -> Self {
        Self {
            id: key_id.to_string(),
            title: key_id_to_title(key_id),
            hold_title: None,
            class: "default".to_string(),
        }
    }

    fn merge(&mut self, opts: &PartialKeyOpts) -> &mut Self {
        assert_eq!(self.id, opts.id);
        if let Some(ref title) = opts.title {
            self.title = title.to_owned();
        }
        if let Some(ref hold_title) = opts.hold_title {
            self.hold_title = Some(hold_title.to_owned());
        }
        if let Some(ref class) = opts.class {
            self.class = class.to_owned();
        }
        self
    }
}

fn key_id_to_title(id: &str) -> String {
    static BASIC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(SE|KC)_([\w\d]|\d+|F\d+)$").unwrap());
    if let Some(basic) = BASIC.captures(id) {
        return basic[2].to_string();
    }
    let res = match id {
        "SE_DOT" => ".",
        "SE_COMM" => ",",
        "SE_SLSH" => "/",
        "SE_LPRN" => "(",
        "SE_RPRN" => ")",
        "SE_UNDS" => "_",
        "SE_TILD" => "~",
        "TILD" => "~",
        "SE_PLUS" => "+",
        "SE_ASTR" => "*",
        "SE_EXLM" => "!",
        "SE_PIPE" => "|",
        "SE_HASH" => "#",
        "SE_AT" => "@",
        "SE_CIRC" => "^",
        "CIRC" => "^",
        "SE_LCBR" => "{",
        "SE_RCBR" => "}",
        "SE_MINS" => "-",
        "SE_BSLS" => "\\",
        "SE_GRV" => "`",
        "GRV" => "`",
        "SE_QUES" => "?",
        "SE_LBRC" => "[",
        "SE_RBRC" => "]",
        "SE_LABK" => "<",
        "SE_RABK" => ">",
        "SE_PERC" => "%",
        "SE_AMPR" => "&",
        "SE_ARNG" => "Å",
        "SE_ADIA" => "Ä",
        "SE_ODIA" => "Ö",
        "SE_ACUT" => "´",
        "SE_DIAE" => "¨",
        "SE_EQL" => "=",
        "SE_DLR" => "$",
        "SE_QUOT" => "'",
        "SE_DQUO" => "\"",
        "SE_SCLN" => ";",
        "KC_UP" => "↑",
        "KC_DOWN" => "↓",
        "KC_LEFT" => "←",
        "KC_RGHT" => "→",
        "KC_HOME" => "Home",
        "KC_END" => "End",
        "KC_PGUP" => "PgUp",
        "KC_PGDN" => "PgDn",
        "KC_BSPC" => "Bspc",
        "KC_DEL" => "Del",
        "KC_ENT" => "Enter",
        "KC_LSFT" => "Shift",
        "KC_RSFT" => "Shift",
        _ => id,
    };
    res.to_string()
}

#[derive(Debug, Clone)]
pub struct PartialKeyOpts {
    pub id: String,
    pub title: Option<String>,
    pub hold_title: Option<String>,
    pub class: Option<String>,
}

impl PartialKeyOpts {
    fn from_spec(key_id: &str, spec: &KeySpec) -> Self {
        Self {
            id: key_id.to_string(),
            title: spec.title.clone(),
            hold_title: spec.hold_title.clone(),
            class: spec.class.clone(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct RenderSpec {
    layers: LayersSpec,
    legend: Vec<LegendSpec>,
    colors: HashMap<String, String>,
    matrix: MatrixSpec,
    combos: CombosSpec,
}

type LayersSpec = HashMap<String, LayerSpec>;
type LayerSpec = Vec<KeySpec>;

#[derive(Deserialize, Debug)]
struct KeySpec {
    keys: Vec<String>,
    title: Option<String>,
    hold_title: Option<String>,
    class: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LegendSpec {
    pub class: String,
    pub title: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MatrixSpec {
    pub left_rows: Vec<usize>,
    pub right_rows: Vec<usize>,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum MatrixHalf {
    Left,
    Right,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct MatrixPos {
    pub x: usize,
    pub y: usize,
    pub half: MatrixHalf,
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct CombosSpec {
    pub background_layer_class: String,
    pub keys_with_separate_imgs: HashSet<String>,
    pub active_class_in_separate_layer: String,
    pub highlight_groups: HashMap<String, HashSet<String>>,
    pub single_img: HashSet<String>,
}

impl MatrixSpec {
    pub fn index_to_matrix_pos(&self, index: usize) -> MatrixPos {
        assert_eq!(self.left_rows.len(), self.right_rows.len());

        let mut curr_i = index;
        for (row_i, (left_count, right_count)) in self
            .left_rows
            .iter()
            .zip(self.right_rows.iter())
            .enumerate()
        {
            if curr_i < *left_count {
                return MatrixPos {
                    x: curr_i,
                    y: row_i,
                    half: MatrixHalf::Left,
                };
            }
            if curr_i < *left_count + *right_count {
                return MatrixPos {
                    x: curr_i,
                    y: row_i,
                    half: MatrixHalf::Right,
                };
            }

            curr_i -= left_count + right_count;
        }
        panic!("Key index overflow: {index}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::Result;

    #[test]
    fn test_parse_render_opts() -> Result<()> {
        let input = r#"
{
  "default": [
    {
      "keys": ["_______", "xxxxxxx"],
      "title": "",
      "class": "blank"
    },
    {
      "keys": ["SE_LPRN"],
      "title": "("
    }
  ],
  "_NUM": [
    {
      "keys": ["SE_J", "SE_P", "SE_K", "AT_U", "SE_LPRN", "SE_RPRN", "NUM_G"],
      "class": "management"
    }
  ]
}
        "#;
        let opts = RenderOpts::parse_from_str(input)?;

        let a = opts.get("_BASE", "SE_A");
        assert_eq!(
            a,
            KeyOpts {
                id: "SE_A".to_string(),
                title: "A".to_string(),
                hold_title: None,
                class: "default".to_string(),
            }
        );

        let lprn = opts.get("_NUM", "SE_LPRN");
        assert_eq!(
            lprn,
            KeyOpts {
                id: "SE_LPRN".to_string(),
                title: "(".to_string(),
                hold_title: None,
                class: "management".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn test_index_to_matrix_pos() {
        let spec = MatrixSpec {
            left_rows: vec![5, 5, 5, 4],
            right_rows: vec![5, 5, 5, 1],
        };

        assert_eq!(
            spec.index_to_matrix_pos(0),
            MatrixPos {
                x: 0,
                y: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(1),
            MatrixPos {
                x: 1,
                y: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(2),
            MatrixPos {
                x: 2,
                y: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(3),
            MatrixPos {
                x: 3,
                y: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(4),
            MatrixPos {
                x: 4,
                y: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(5),
            MatrixPos {
                x: 5,
                y: 0,
                half: MatrixHalf::Right
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(6),
            MatrixPos {
                x: 6,
                y: 0,
                half: MatrixHalf::Right
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(7),
            MatrixPos {
                x: 7,
                y: 0,
                half: MatrixHalf::Right
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(8),
            MatrixPos {
                x: 8,
                y: 0,
                half: MatrixHalf::Right
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(9),
            MatrixPos {
                x: 9,
                y: 0,
                half: MatrixHalf::Right
            }
        );

        assert_eq!(
            spec.index_to_matrix_pos(10),
            MatrixPos {
                x: 0,
                y: 1,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(19),
            MatrixPos {
                x: 9,
                y: 1,
                half: MatrixHalf::Right
            }
        );

        assert_eq!(
            spec.index_to_matrix_pos(20),
            MatrixPos {
                x: 0,
                y: 2,
                half: MatrixHalf::Left
            }
        );

        assert_eq!(
            spec.index_to_matrix_pos(30),
            MatrixPos {
                x: 0,
                y: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(31),
            MatrixPos {
                x: 1,
                y: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(32),
            MatrixPos {
                x: 2,
                y: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(33),
            MatrixPos {
                x: 3,
                y: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            spec.index_to_matrix_pos(34),
            MatrixPos {
                x: 4,
                y: 3,
                half: MatrixHalf::Right
            }
        );
    }
}
