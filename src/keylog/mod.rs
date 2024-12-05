use crate::parse::Combo;
use crate::parse::FingerAssignment;
use crate::parse::InputInfo;
use crate::parse::Key;
use crate::parse::LayerId;
use crate::parse::MatrixHalf;
use camino::Utf8Path;
use csv::ReaderBuilder;
use eyre::{OptionExt, Result};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;

pub fn output_stats(info: &InputInfo, keylog_file: &Utf8Path) -> Result<()> {
    let stats = KeylogStats::from_file(info, keylog_file)?;

    let mut list: Vec<_> = stats
        .output_frequency
        .iter()
        .map(|(key, freq)| (freq, key))
        .collect();
    list.sort();
    for (freq, key) in list {
        println!("{key:>10}: {freq}");
    }

    let mut finger_row = String::new();
    let mut stats_row = String::new();
    for (x, freq) in &stats.finger_frequency {
        finger_row.push_str(&format!("{:>8}", x.finger.to_string()));
        let perc = (*freq) as f32 / stats.total_presses as f32 * 100.0;
        stats_row.push_str(&format!("{perc:>7.2}%"));
    }
    println!();
    println!("{}", finger_row);
    println!("{}", stats_row);
    println!();

    let left = stats.total_left as f32 / stats.total_presses as f32 * 100.0;
    println!("    left: {left:>7.2}%");
    let right = stats.total_right as f32 / stats.total_presses as f32 * 100.0;
    println!("   right: {right:>7.2}%");

    output_sfbs(&stats, "sfbs (without combos)", false);
    output_sfbs(&stats, "sfbs (with combos)", true);

    Ok(())
}

fn output_sfbs(stats: &KeylogStats, title: &str, combos: bool) {
    let mut finger_row = String::new();
    let mut stats_row = String::new();
    let mut total_presses = 0;
    for (finger, sfbs_by_id) in &stats.sfbs_by_finger {
        finger_row.push_str(&format!("{:>8}", finger.finger.to_string()));
        let presses: u32 = sfbs_by_id
            .values()
            .filter(|x| if !combos { !x.sfb.has_combo() } else { true })
            .map(|x| x.presses)
            .sum();
        total_presses += presses;
        let perc = presses as f32 / stats.total_presses as f32 * 100.0;
        stats_row.push_str(&format!("{perc:>7.2}%"));
    }
    println!();
    println!();
    println!("  {title}");
    println!("{}", finger_row);
    println!("{}", stats_row);
    println!();
    let perc = total_presses as f32 / stats.total_presses as f32 * 100.0;
    println!("  total: {perc:>7.3}%",);

    let top_sfbs = stats
        .sfbs
        .iter()
        .rev()
        .filter(|x| if !combos { !x.sfb.has_combo() } else { true })
        .take(10);

    println!("  top sfbs:");
    for sfb in top_sfbs {
        let perc = sfb.presses as f32 / stats.total_presses as f32 * 100.0;
        println!("   {:<35}     {perc:>.2}%", sfb.sfb.id());
    }
}

#[derive(Debug)]
struct KeylogStats {
    output_frequency: HashMap<String, u32>,
    finger_frequency: BTreeMap<FingerAssignment, u32>,
    total_presses: u32,
    total_left: u32,
    total_right: u32,
    total_sfbs: u32,
    sfbs: Vec<SfbStats>,
    sfbs_by_finger: BTreeMap<FingerAssignment, HashMap<String, SfbStats>>,
    sfbs_by_id: HashMap<String, SfbStats>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct SfbStats {
    presses: u32,
    sfb: Sfb,
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
    fn from_file(info: &InputInfo, keylog_file: &Utf8Path) -> Result<Self> {
        let raw_entries = parse_csv(keylog_file)?;
        Self::from_entries(info, raw_entries)
    }

    fn from_entries(info: &InputInfo, raw_entries: Vec<RawKeylogEntry>) -> Result<Self> {
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
                            .and_modify(|x| x.presses += 1)
                            .or_insert_with(|| SfbStats {
                                presses: 1,
                                sfb: sfb.sfb.clone(),
                            });
                    })
                    .or_insert_with(|| {
                        [(
                            sfb.sfb.id(),
                            SfbStats {
                                presses: 1,
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
            total_sfbs: sfb_series.len() as u32,
            output_frequency: frequency,
            finger_frequency,
            total_presses,
            total_left,
            total_right,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Sfb {
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

    fn has_combo(&self) -> bool {
        matches!(self, Self::Combo { .. })
    }

    fn id(&self) -> String {
        format!(
            "{:>22}    {:<20}",
            self.first_ids_to_string(),
            self.second_ids_to_string()
        )
    }

    fn first_ids_to_string(&self) -> String {
        match self {
            Self::Combo { first_keys, .. } => {
                let v: Vec<&str> = first_keys.iter().map(|key| key.id.0.as_str()).collect();
                v.join(",")
            }
            Self::Single { first_key, .. } => first_key.id.to_string(),
        }
    }

    fn second_ids_to_string(&self) -> String {
        match self {
            Self::Combo { second_keys, .. } => {
                let v: Vec<&str> = second_keys.iter().map(|key| key.id.0.as_str()).collect();
                v.join(",")
            }
            Self::Single { second_key, .. } => second_key.id.to_string(),
        }
    }

    fn get_fingers(&self) -> HashSet<FingerAssignment> {
        match self {
            Self::Combo { fingers, .. } => fingers.clone(),
            Self::Single { finger, .. } => [*finger].into_iter().collect(),
        }
    }
}

#[derive(Debug)]
enum KeylogEntry<'a> {
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
    // pub fn ids_to_string(&self) -> String {
    //     match self {
    //         KeylogEntry::Combo(combo) => {
    //             let v: Vec<&str> = combo.keys.iter().map(|key| key.id.0.as_str()).collect();
    //             v.join(",")
    //         }
    //         KeylogEntry::Single { key, .. } => key.id.0.clone(),
    //     }
    // }

    pub fn is_key_sfb(&self, key: &Key) -> bool {
        match self {
            KeylogEntry::Combo(combo) => {
                combo.is_key_sfb(key)
                // if combo.keys.iter().any(|combo_key| key.physical_pos)
                // combo.keys.iter().any(|combo_key| key.is_sfb(combo_key))
            }
            KeylogEntry::Single { key: other, .. } => key.is_sfb(other),
        }
    }

    pub fn is_combo_sfb(&self, combo: &Combo) -> bool {
        // if
        // combo
        // .keys
        // .iter()
        // .any(|combo_key| other.is_key_sfb(combo_key))
        match self {
            KeylogEntry::Combo(combo) => combo.is_combo_sfb(combo),
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

#[derive(Debug, Deserialize)]
struct RawKeylogEntry {
    keycode: String, // hex or COMBO
    row: String,
    col: String,
    highest_layer: usize,
    pressed: usize,
    mods: String,         // hex
    oneshot_mods: String, // hex
    tap_count: usize,     // or combo_index
}

fn parse_csv(keylog_file: &Utf8Path) -> Result<Vec<RawKeylogEntry>> {
    let file = File::open(keylog_file)?;

    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(file);

    let mut res = Vec::new();
    for row in rdr.deserialize() {
        let entry: RawKeylogEntry = row?;
        res.push(entry);
    }
    // dbg!(&res);
    Ok(res)
}
