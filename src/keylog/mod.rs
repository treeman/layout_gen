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
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;

pub fn output_stats(info: &InputInfo, keylog_file: &Utf8Path) -> Result<()> {
    let raw_entries = parse_csv(keylog_file)?;
    let entries = convert_keylog_entries(&raw_entries, info)?;

    let mut frequency = HashMap::new();
    let mut finger_frequency = BTreeMap::new();

    for entry in &entries {
        match entry {
            KeylogEntry::Combo(combo) => {
                frequency
                    .entry(combo.output.as_str())
                    .and_modify(|x| *x += 1)
                    .or_insert(1);
                for key in &combo.keys {
                    let finger = info.render_opts.assigned_finger(key.physical_pos.pos());
                    finger_frequency
                        .entry(FingerAssignment {
                            half: key.physical_pos.half,
                            finger,
                        })
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                }
            }
            KeylogEntry::Single { key, .. } => {
                frequency
                    .entry(key.id.0.as_str())
                    .and_modify(|x| *x += 1)
                    .or_insert(1);
                let finger = info.render_opts.assigned_finger(key.physical_pos.pos());
                finger_frequency
                    .entry(FingerAssignment {
                        half: key.physical_pos.half,
                        finger,
                    })
                    .and_modify(|x| *x += 1)
                    .or_insert(1);
            }
        }
    }

    let mut list: Vec<_> = frequency.iter().map(|(key, freq)| (freq, key)).collect();
    list.sort();
    for (freq, key) in list {
        println!("{key:>10}: {freq}");
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

    let mut finger_row = String::new();
    let mut stats_row = String::new();
    for (x, freq) in &finger_frequency {
        finger_row.push_str(&format!("{:>8}", x.finger.to_string()));
        let perc = (*freq) as f32 / total_presses as f32 * 100.0;
        stats_row.push_str(&format!("{perc:>7.2}%"));
    }
    println!();
    println!("{}", finger_row);
    println!("{}", stats_row);
    println!();

    let left = total_left as f32 / total_presses as f32 * 100.0;
    println!("    left: {left:>7.2}%");
    let right = total_right as f32 / total_presses as f32 * 100.0;
    println!("   right: {right:>7.2}%");

    Ok(())
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
