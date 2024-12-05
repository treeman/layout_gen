use super::csv_parser::{self, RawKeylogEntry};
use crate::parse::Combo;
use crate::parse::Finger;
use crate::parse::FingerAssignment;
use crate::parse::InputInfo;
use crate::parse::Key;
use crate::parse::LayerId;
use crate::parse::MatrixHalf;
use camino::Utf8Path;
use eyre::{OptionExt, Result};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug)]
pub struct KeylogStats {
    pub output_frequency: HashMap<String, u32>,
    pub finger_frequency: BTreeMap<FingerAssignment, u32>,
    // One combo produces a single event (relevant for sfb calculations)
    pub total_events: u32,
    // Note that one combo can produce multiple key presses
    pub total_key_presses: u32,
    pub total_key_presses_left: u32,
    pub total_key_presses_right: u32,
    pub total_sfb_events: u32,
    pub sfbs: Vec<SfbStats>,
    pub sfbs_by_finger: BTreeMap<FingerAssignment, HashMap<String, SfbStats>>,
    pub sfbs_by_id: HashMap<String, SfbStats>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SfbStats {
    pub presses: u32,
    pub sfb: Sfb,
}

impl PartialOrd for SfbStats {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other)) // Delegate to cmp
    }
}

impl Ord for SfbStats {
    fn cmp(&self, other: &Self) -> Ordering {
        self.presses.cmp(&other.presses)
    }
}

impl KeylogStats {
    pub fn from_file(info: &InputInfo, keylog_file: &Utf8Path) -> Result<Self> {
        let raw_entries = csv_parser::parse(keylog_file)?;
        Self::from_entries(info, raw_entries)
    }

    pub fn from_entries(info: &InputInfo, raw_entries: Vec<RawKeylogEntry>) -> Result<Self> {
        let entries = convert_keylog_entries(&raw_entries, info)?;

        let mut frequency = HashMap::new();
        let mut finger_frequency = BTreeMap::new();

        for entry in &entries {
            match entry {
                KeylogEntry::Combo(combo) => {
                    frequency
                        .entry(combo.output.to_string())
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                    for key in &combo.keys {
                        finger_frequency
                            .entry(key.physical_pos.finger)
                            .and_modify(|x| *x += 1)
                            .or_insert(1);
                    }
                }
                KeylogEntry::Single { key, .. } => {
                    frequency
                        .entry(key.id.0.to_string())
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                    finger_frequency
                        .entry(key.physical_pos.finger)
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                }
            }
        }

        let mut total_presses = 0;
        let mut total_left = 0;
        let mut total_right = 0;
        for (x, freq) in &finger_frequency {
            total_presses += freq;
            match x.half {
                MatrixHalf::Left => total_left += freq,
                MatrixHalf::Right => total_right += freq,
            }
        }

        let sfb_series: Vec<Sfb> = entries
            .iter()
            .zip(entries.iter().skip(1))
            .filter_map(|(current, next)| Sfb::new_if_sfb(current, next))
            .collect();

        let mut sfbs_by_id: HashMap<String, SfbStats> = HashMap::new();
        for sfb in &sfb_series {
            println!("{}", sfb.id());
            sfbs_by_id
                .entry(sfb.id())
                .and_modify(|x| x.presses += 1)
                .or_insert_with(|| SfbStats {
                    presses: 1,
                    sfb: sfb.clone(),
                });
        }

        let mut sfbs: Vec<SfbStats> = Vec::new();
        let mut sfbs_by_finger: BTreeMap<FingerAssignment, HashMap<String, SfbStats>> =
            BTreeMap::new();
        for (_id, sfb) in sfbs_by_id.iter() {
            sfbs.push(sfb.clone());
            for finger in sfb.sfb.get_fingers() {
                sfbs_by_finger
                    .entry(finger)
                    .and_modify(|x| {
                        x.entry(sfb.sfb.id())
                            .and_modify(|x| x.presses += sfb.presses)
                            .or_insert_with(|| SfbStats {
                                presses: sfb.presses,
                                sfb: sfb.sfb.clone(),
                            });
                    })
                    .or_insert_with(|| {
                        [(
                            sfb.sfb.id(),
                            SfbStats {
                                presses: sfb.presses,
                                sfb: sfb.sfb.clone(),
                            },
                        )]
                        .into_iter()
                        .collect()
                    });
            }
        }
        sfbs.sort();

        Ok(Self {
            sfbs,
            sfbs_by_id,
            sfbs_by_finger,
            total_events: entries.len() as u32,
            total_sfb_events: sfb_series.len() as u32,
            output_frequency: frequency,
            finger_frequency,
            total_key_presses: total_presses,
            total_key_presses_left: total_left,
            total_key_presses_right: total_right,
        })
    }

    pub fn top_sfbs(&self, count: usize, include_combos: bool) -> impl Iterator<Item = &SfbStats> {
        self.sfbs
            .iter()
            .rev()
            .filter(move |x| {
                if !include_combos {
                    !x.sfb.has_combo()
                } else {
                    true
                }
            })
            .take(count)
    }

    pub fn sfb_frequency_by_finger(&self, include_combos: bool) -> BTreeMap<FingerAssignment, u32> {
        self.sfbs_by_finger
            .iter()
            .map(|(finger, sfbs_by_id)| {
                let presses: u32 = sfbs_by_id
                    .values()
                    .filter(move |x| {
                        if !include_combos {
                            !x.sfb.has_combo()
                        } else {
                            true
                        }
                    })
                    .map(|x| x.presses)
                    .sum();
                (*finger, presses)
            })
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Sfb {
    Combo {
        first_keys: Vec<Key>,
        second_keys: Vec<Key>,
        fingers: HashSet<FingerAssignment>,
    },
    Single {
        first_key: Key,
        second_key: Key,
        finger: FingerAssignment,
    },
}

impl Sfb {
    fn new_if_sfb(current: &KeylogEntry<'_>, next: &KeylogEntry<'_>) -> Option<Self> {
        if !current.is_entry_sfb(next) {
            return None;
        }

        let res = match (current, next) {
            (KeylogEntry::Combo(current_combo), KeylogEntry::Combo(next_combo)) => {
                let mut fingers = current_combo.get_fingers();
                fingers.extend(next_combo.get_fingers().iter());
                Self::Combo {
                    first_keys: current_combo.keys.iter().map(Clone::clone).collect(),
                    second_keys: next_combo.keys.iter().map(Clone::clone).collect(),
                    fingers,
                }
            }
            (KeylogEntry::Combo(combo), KeylogEntry::Single { key, .. }) => {
                let mut fingers = combo.get_fingers();
                fingers.insert(key.physical_pos.finger);
                Self::Combo {
                    first_keys: combo.keys.iter().map(Clone::clone).collect(),
                    second_keys: vec![(*key).clone()],
                    fingers,
                }
            }
            (KeylogEntry::Single { key, .. }, KeylogEntry::Combo(combo)) => {
                let mut fingers = combo.get_fingers();
                fingers.insert(key.physical_pos.finger);
                Self::Combo {
                    first_keys: vec![(*key).clone()],
                    second_keys: combo.keys.iter().map(Clone::clone).collect(),
                    fingers,
                }
            }
            (
                KeylogEntry::Single {
                    key: current_key, ..
                },
                KeylogEntry::Single { key: next_key, .. },
            ) => Self::Single {
                first_key: (*current_key).clone(),
                second_key: (*next_key).clone(),
                finger: current_key.physical_pos.finger,
            },
        };

        Some(res)
    }

    pub fn has_combo(&self) -> bool {
        matches!(self, Self::Combo { .. })
    }

    pub fn id(&self) -> String {
        format!(
            "{:>22}    {:<20}",
            self.first_ids_to_string(),
            self.second_ids_to_string()
        )
    }

    pub fn first_ids_to_string(&self) -> String {
        match self {
            Self::Combo { first_keys, .. } => {
                let v: Vec<&str> = first_keys.iter().map(|key| key.id.0.as_str()).collect();
                v.join(",")
            }
            Self::Single { first_key, .. } => first_key.id.to_string(),
        }
    }

    pub fn second_ids_to_string(&self) -> String {
        match self {
            Self::Combo { second_keys, .. } => {
                let v: Vec<&str> = second_keys.iter().map(|key| key.id.0.as_str()).collect();
                v.join(",")
            }
            Self::Single { second_key, .. } => second_key.id.to_string(),
        }
    }

    pub fn get_fingers(&self) -> HashSet<FingerAssignment> {
        match self {
            Self::Combo { fingers, .. } => fingers.clone(),
            Self::Single { finger, .. } => [*finger].into_iter().collect(),
        }
    }
}

#[derive(Debug)]
pub enum KeylogEntry<'a> {
    Combo(&'a Combo),
    Single {
        key: &'a Key,
        keycode: String,
        highest_layer: LayerId,
        pressed: bool,
        // mods, oneshot_mods
        tap_count: usize,
    },
}

impl KeylogEntry<'_> {
    pub fn is_key_sfb(&self, key: &Key) -> bool {
        match self {
            KeylogEntry::Combo(combo) => combo.is_key_sfb(key),
            KeylogEntry::Single { key: other, .. } => key.is_sfb(other),
        }
    }

    pub fn is_combo_sfb(&self, combo: &Combo) -> bool {
        match self {
            KeylogEntry::Combo(my_combo) => my_combo.is_combo_sfb(combo),
            KeylogEntry::Single { key, .. } => combo.is_key_sfb(key),
        }
    }

    pub fn is_entry_sfb(&self, other: &KeylogEntry) -> bool {
        match self {
            KeylogEntry::Combo(combo) => other.is_combo_sfb(combo),
            KeylogEntry::Single { key, .. } => other.is_key_sfb(key),
        }
    }
}

fn convert_keylog_entries<'a>(
    entries: &[RawKeylogEntry],
    info: &'a InputInfo,
) -> Result<Vec<KeylogEntry<'a>>> {
    let mut res = Vec::with_capacity(entries.len());

    for entry in entries {
        if entry.keycode == "COMBO" {
            let combo = info
                .keymap
                .combos
                .get(entry.tap_count)
                .expect("Combo index out of bounds");

            res.push(KeylogEntry::Combo(combo));
            continue;
        }
        let pressed = entry.pressed != 0;
        if !pressed {
            continue;
        }
        let row = entry.row.parse()?;
        let col = entry.col.parse()?;

        if row == 254 && col == 254 {
            continue;
        }

        // TODO fetch from specific layer
        let key = match info
            .keymap
            .find_key_by_matrix(entry.highest_layer, (row, col))
        {
            Some(key) => key,
            None => {
                panic!("Could not find key for position {} {}", row, col);
            }
        };

        let highest_layer = info
            .keymap
            .get_layer_id(entry.highest_layer)
            .ok_or_eyre(format!(
                "Layer out of bounds {} > {}",
                entry.highest_layer,
                info.keymap.layers.len()
            ))?;

        res.push(KeylogEntry::Single {
            keycode: entry.keycode.clone(),
            key,
            highest_layer,
            pressed,
            tap_count: entry.tap_count,
        });
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::*;

    #[test]
    fn test_sfb_stats() -> Result<()> {
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
COMB(coln_sym,          COLN_SYM,       SE_N, SE_A)
        "##;

        let render_input = r#"
{
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
  ],
  "layers": {
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
}
        "#;

        let render_opts = RenderOpts::parse_from_str("id", render_input)?;
        let keymap = Keymap::parse_from_source(keymap_c, keyboard_json, combos_def, &render_opts)?;

        let info = InputInfo {
            keymap,
            render_opts,
        };

        // 2nd + 3rd for a regular keylog entry is the matrix position and the 5th needs to be 1
        // (pressed)
        // For a COMBO, the last entry is the combo index from combo.def
        let keylog = [
            // MT_SPC
            "0x0001,3,4,0,1,0x00,0x00,1",
            // Both thumb keys, no sfb because it's the same
            "COMBO,NA,NA,0,0,0,0,0",
            // SE_J
            "0x0001,1,0,0,1,0x00,0x00,1",
            // SE_C, sfb using ring
            "0x0001,0,1,0,1,0x00,0x00,1",
            // SE_S, sfb with C
            "0x0001,1,1,0,1,0x00,0x00,1",
            "0x0001,1,1,0,1,0x00,0x00,1",
            "0x0001,1,1,0,1,0x00,0x00,1",
            // SE_C, sfb with S
            "0x0001,0,1,0,1,0x00,0x00,1",
            // SE_S, sfb with C
            "0x0001,1,1,0,1,0x00,0x00,1",
            // SE_T
            "0x0001,1,2,0,1,0x00,0x00,1",
            // ESC SYM, no sfb as it uses same key
            "COMBO,NA,NA,0,0,0,0,3",
            // <=, no sfb as it uses same key (but maybe it should be...?)
            "COMBO,NA,NA,0,0,0,0,4",
            // SE_L
            "0x0001,6,1,0,1,0x00,0x00,1",
            // SE_W sfb
            "0x0001,4,1,0,1,0x00,0x00,1",
            // sfb :
            "COMBO,NA,NA,0,0,0,0,6",
            // sfb boot
            "COMBO,NA,NA,0,0,0,0,2",
            // sfb :
            "COMBO,NA,NA,0,0,0,0,6",
        ]
        .join("\n");
        let entries = csv_parser::parse_from_str(&keylog)?;

        let stats = KeylogStats::from_entries(&info, entries)?;

        assert_eq!(stats.total_sfb_events, 8);
        assert_eq!(stats.total_events, 17);
        assert_eq!(stats.total_key_presses, 26);

        assert_eq!(
            stats.finger_frequency.get(&FingerAssignment {
                finger: Finger::Pinky,
                half: MatrixHalf::Left,
            }),
            None
        );
        assert_eq!(
            stats.finger_frequency.get(&FingerAssignment {
                finger: Finger::Ring,
                half: MatrixHalf::Left,
            }),
            Some(&7)
        );
        assert_eq!(
            stats.finger_frequency.get(&FingerAssignment {
                finger: Finger::Index,
                half: MatrixHalf::Right,
            }),
            Some(&5)
        );

        let sfb_frequency_by_finger = stats.sfb_frequency_by_finger(true);

        assert_eq!(
            sfb_frequency_by_finger.get(&FingerAssignment {
                finger: Finger::Pinky,
                half: MatrixHalf::Left,
            }),
            None
        );
        assert_eq!(
            sfb_frequency_by_finger.get(&FingerAssignment {
                finger: Finger::Ring,
                half: MatrixHalf::Left,
            }),
            Some(&4)
        );
        assert_eq!(
            sfb_frequency_by_finger.get(&FingerAssignment {
                finger: Finger::Index,
                half: MatrixHalf::Right,
            }),
            Some(&4)
        );

        Ok(())
    }

    #[test]
    fn test_is_sfb() {
        let combo_a = Combo {
            id: "comb_boot_r".into(),
            output: "QK_BOOT".into(),
            keys: vec![
                Key {
                    id: KeyId("SE_E".into()),
                    x: 6.5,
                    y: 4.0,
                    physical_pos: PhysicalPos {
                        col: 4,
                        row: 4,
                        finger: FingerAssignment {
                            finger: Finger::Thumb,
                            half: MatrixHalf::Right,
                        },
                        effort: 0,
                    },
                    matrix_pos: (7, 0),
                },
                Key {
                    id: KeyId("SE_L".into()),
                    x: 8.0,
                    y: 2.28,
                    physical_pos: PhysicalPos {
                        col: 6,
                        row: 2,
                        finger: FingerAssignment {
                            finger: Finger::Index,
                            half: MatrixHalf::Right,
                        },
                        effort: 3,
                    },
                    matrix_pos: (6, 1),
                },
                Key {
                    id: KeyId("SE_LPRN".into()),
                    x: 9.0,
                    y: 2.0,
                    physical_pos: PhysicalPos {
                        col: 7,
                        row: 2,
                        finger: FingerAssignment {
                            finger: Finger::Middle,
                            half: MatrixHalf::Right,
                        },
                        effort: 4,
                    },
                    matrix_pos: (6, 2),
                },
                Key {
                    id: KeyId("SE_RPRN".into()),
                    x: 10.0,
                    y: 2.31,
                    physical_pos: PhysicalPos {
                        col: 8,
                        row: 2,
                        finger: FingerAssignment {
                            finger: Finger::Ring,
                            half: MatrixHalf::Right,
                        },
                        effort: 4,
                    },
                    matrix_pos: (6, 3),
                },
                Key {
                    id: KeyId("SE_UNDS".into()),
                    x: 11.0,
                    y: 2.93,
                    physical_pos: PhysicalPos {
                        col: 9,
                        row: 2,
                        finger: FingerAssignment {
                            finger: Finger::Pinky,
                            half: MatrixHalf::Right,
                        },
                        effort: 6,
                    },
                    matrix_pos: (6, 4),
                },
            ],
        };
        let a = KeylogEntry::Combo(&combo_a);

        let combo_b = Combo {
            id: "combo_coln".into(),
            output: "SE_COLN".into(),
            keys: vec![
                Key {
                    id: KeyId("SE_R".into()),
                    x: 0.0,
                    y: 1.93,
                    physical_pos: PhysicalPos {
                        col: 0,
                        row: 1,
                        finger: FingerAssignment {
                            finger: Finger::Pinky,
                            half: MatrixHalf::Left,
                        },
                        effort: 2,
                    },
                    matrix_pos: (2, 0),
                },
                Key {
                    id: KeyId("SE_M".into()),
                    x: 7.0,
                    y: 1.42,
                    physical_pos: PhysicalPos {
                        col: 0,
                        row: 1,
                        finger: FingerAssignment {
                            finger: Finger::Index,
                            half: MatrixHalf::Right,
                        },
                        effort: 5,
                    },
                    matrix_pos: (5, 0),
                },
            ],
        };
        let b = KeylogEntry::Combo(&combo_b);

        assert!(a.is_entry_sfb(&b));
    }
}
