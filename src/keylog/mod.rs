use crate::parse::InputInfo;
use crate::parse::Key;
use crate::parse::LayerId;
use camino::Utf8Path;
use csv::ReaderBuilder;
use eyre::{OptionExt, Result};
use serde::Deserialize;
use std::fs::File;

pub fn output_stats(info: &InputInfo, keylog_file: &Utf8Path) -> Result<()> {
    let raw_entries = parse_csv(keylog_file)?;
    let entries = convert_keylog_entries(&raw_entries, info)?;
    dbg!(entries.len());
    Ok(())
}

#[derive(Debug)]
struct KeylogEntry<'a> {
    key: &'a Key,
    highest_layer: LayerId,
    pressed: bool,
    // mods, oneshot_mods
    tap_count: usize,
}

fn convert_keylog_entries<'a>(
    entries: &[RawKeylogEntry],
    info: &'a InputInfo,
) -> Result<Vec<KeylogEntry<'a>>> {
    let mut res = Vec::with_capacity(entries.len());

    for entry in entries {
        if entry.row == "NA" {
            // TODO handle combos
            continue;
        }
        let row = entry.row.parse()?;
        let col = entry.col.parse()?;

        if row == 254 && col == 254 {
            // TODO handle COMBO_EVENT
            // println!("Found combo {}", entry.keycode);
            // let code: u32 = u32::from_str_radix(&entry.keycode.strip_prefix("0x").unwrap(), 16)?;
            // for b in code.to_ne_bytes() {
            //     println!("  {b}")
            // }
            continue;
        }

        let key = match info.keymap.matrix_lookup.get(col, row) {
            Some(key) => key,
            None => {
                // println!("Could not find key for position {} {}", row, col);
                // dbg!(&info.keymap.matrix_lookup.keys);
                continue;
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

        res.push(KeylogEntry {
            key,
            highest_layer,
            pressed: entry.pressed != 1,
            tap_count: entry.tap_count,
        });
    }

    // TODO
    // Top left corner is [1, 0] (need to accurately follow matrix spec)
    // Top left of right-side keyboard is [4, 0] (it sets rows below)
    // let mut out: Vec<String> = Vec::new();
    // for ((x, y), key) in info.keymap.matrix_lookup.keys.iter() {
    //     out.push(format!("  {x} {y} {}", key.id.0));
    // }
    // out.sort();
    // for x in out {
    //     println!("{x}");
    // }

    // dbg!(&info.keymap.matrix_lookup);

    Ok(res)
}

#[derive(Debug, Deserialize)]
struct RawKeylogEntry {
    keycode: String, // hex
    row: String,     // or 254 if combo OR NA if combo
    col: String,     // or 254 if combo OR NA if combo
    highest_layer: usize,
    pressed: usize,
    mods: String,         // hex
    oneshot_mods: String, // hex
    tap_count: usize,
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
