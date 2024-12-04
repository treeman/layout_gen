use camino::Utf8Path;
use eyre::Result;
use regex::Regex;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct RenderOpts {
    pub id: String,
    pub default_keys: HashMap<String, PartialKeyOpts>,
    pub layer_keys: HashMap<String, HashMap<String, PartialKeyOpts>>,
    pub legend: Vec<LegendSpec>,
    pub colors: HashMap<String, String>,
    pub physical_layout: PhysicalLayout,
    pub finger_assignmens: PhysicalLayout,
    pub outputs: RenderOutputs,
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
            physical_layout: spec.physical_layout.convert(),
            finger_assignmens: spec.finger_assignments.convert(),
            outputs: spec.outputs,
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

    pub fn assigned_finger(&self, pos: (usize, usize)) -> Finger {
        let spec = self.finger_assignmens.get(pos);
        match spec.value {
            0 => Finger::Pinky,
            1 => Finger::Ring,
            2 => Finger::Middle,
            3 => Finger::Index,
            4 => Finger::Thumb,
            _ => panic!("Finger value {} unknown", spec.value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FingerAssignment {
    pub finger: Finger,
    pub half: MatrixHalf,
}

impl PartialOrd for FingerAssignment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other)) // Delegate to cmp
    }
}

impl Ord for FingerAssignment {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.half, other.half) {
            (MatrixHalf::Left, MatrixHalf::Left) => self.finger.cmp(&other.finger),
            (MatrixHalf::Right, MatrixHalf::Right) => self.finger.cmp(&other.finger).reverse(),
            _ => self.half.cmp(&other.half),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

impl std::fmt::Display for Finger {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Finger::Pinky => "pinky",
            Finger::Ring => "ring",
            Finger::Middle => "middle",
            Finger::Index => "index",
            Finger::Thumb => "thumb",
        };
        write!(f, "{}", s)
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
        "SE_COLN" => ":",
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
        "KC_ESC" => "Esc",
        "KC_TAB" => "Tab",
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
    physical_layout: PhysicalLayoutSpec,
    finger_assignments: PhysicalLayoutSpec,
    outputs: RenderOutputs,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum MatrixHalf {
    Left,
    Right,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RenderOutputs {
    #[serde(default)]
    pub effort: bool,
    #[serde(default = "default_true")]
    pub layers: bool,
    #[serde(default = "default_true")]
    pub legend: bool,
    #[serde(default = "default_true")]
    pub combos: bool,
    pub combo_keys_with_separate_imgs: HashSet<String>,
    pub combo_highlight_groups: HashMap<String, HashSet<String>>,
    pub combo_background_layer_class: String,
    pub active_class_in_separate_layer: String,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone)]
struct PhysicalLayoutSpec(Vec<String>);

impl PhysicalLayoutSpec {
    fn convert(self) -> PhysicalLayout {
        let mut index_to_pos = Vec::new();

        for (row, line) in self.0.into_iter().enumerate() {
            let split: Vec<_> = line.trim_end().split("    ").collect();
            assert!(split.len() <= 2);

            let mut curr_col = 0;
            for char in split[0].chars() {
                if char != ' ' {
                    index_to_pos.push(PhysicalPos {
                        col: curr_col,
                        row,
                        half: MatrixHalf::Left,
                        value: char
                            .to_digit(10)
                            .expect("Physical layout should contain digits"),
                    });
                }
                curr_col += 1;
            }

            if split.len() > 1 {
                for char in split[1].chars() {
                    if char != ' ' {
                        index_to_pos.push(PhysicalPos {
                            col: curr_col,
                            row,
                            half: MatrixHalf::Right,
                            value: char
                                .to_digit(10)
                                .expect("Physical layout should contain digits"),
                        });
                    }
                    curr_col += 1;
                }
            }
        }

        let pos_to_index = index_to_pos
            .iter()
            .enumerate()
            .map(|x: (usize, &PhysicalPos)| ((x.1.col, x.1.row), x.0))
            .collect();

        PhysicalLayout {
            index_to_pos,
            pos_to_index,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PhysicalLayout {
    index_to_pos: Vec<PhysicalPos>,
    pos_to_index: HashMap<(usize, usize), usize>,
}

impl PhysicalLayout {
    pub fn index_to_pos(&self, index: usize) -> PhysicalPos {
        assert!(index <= self.index_to_pos.len());
        self.index_to_pos[index]
    }

    pub fn get(&self, pos: (usize, usize)) -> PhysicalPos {
        let index = self
            .pos_to_index
            .get(&pos)
            .unwrap_or_else(|| panic!("Couldn't map {pos:?} to index"));
        self.index_to_pos(*index)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct PhysicalPos {
    pub col: usize,
    pub row: usize,
    pub half: MatrixHalf,
    pub value: u32,
}

impl PhysicalPos {
    pub fn pos(&self) -> (usize, usize) {
        (self.col, self.row)
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
        let opts = RenderOpts::parse_from_str("id", input)?;

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
    fn test_physical_layout() {
        let spec = PhysicalLayoutSpec(vec![
            "54446    64445".into(),
            "21005    50012".into(),
            "64436    63446".into(),
            " 77".into(),
            "   80    0".into(),
        ]);
        let layout = spec.convert();
        assert_eq!(
            layout.index_to_pos(0),
            PhysicalPos {
                col: 0,
                row: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            layout.index_to_pos(1),
            PhysicalPos {
                col: 1,
                row: 0,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            layout.index_to_pos(5),
            PhysicalPos {
                col: 5,
                row: 0,
                half: MatrixHalf::Right
            }
        );
        assert_eq!(
            layout.index_to_pos(9),
            PhysicalPos {
                col: 9,
                row: 0,
                half: MatrixHalf::Right
            }
        );

        assert_eq!(
            layout.index_to_pos(10),
            PhysicalPos {
                col: 0,
                row: 1,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            layout.index_to_pos(11),
            PhysicalPos {
                col: 1,
                row: 1,
                half: MatrixHalf::Left
            }
        );

        assert_eq!(
            layout.index_to_pos(20),
            PhysicalPos {
                col: 0,
                row: 2,
                half: MatrixHalf::Left
            }
        );

        assert_eq!(
            layout.index_to_pos(30),
            PhysicalPos {
                col: 1,
                row: 3,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            layout.index_to_pos(31),
            PhysicalPos {
                col: 2,
                row: 3,
                half: MatrixHalf::Left
            }
        );

        assert_eq!(
            layout.index_to_pos(32),
            PhysicalPos {
                col: 3,
                row: 4,
                half: MatrixHalf::Left
            }
        );
        assert_eq!(
            layout.index_to_pos(33),
            PhysicalPos {
                col: 4,
                row: 4,
                half: MatrixHalf::Left
            }
        );

        assert_eq!(
            layout.index_to_pos(34),
            PhysicalPos {
                col: 5,
                row: 4,
                half: MatrixHalf::Right
            }
        );
    }
}
