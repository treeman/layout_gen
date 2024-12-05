use camino::Utf8Path;
use csv::ReaderBuilder;
use eyre::Result;
use serde::Deserialize;
use std::fs::File;
use std::io::Cursor;

#[derive(Debug, Deserialize)]
pub struct RawKeylogEntry {
    pub keycode: String, // hex or COMBO
    pub row: String,
    pub col: String,
    pub highest_layer: usize,
    pub pressed: usize,
    pub mods: String,         // hex
    pub oneshot_mods: String, // hex
    pub tap_count: usize,     // or combo_index
}

pub fn parse(keylog_file: &Utf8Path) -> Result<Vec<RawKeylogEntry>> {
    let file = File::open(keylog_file)?;

    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(file);

    let mut res = Vec::new();
    for row in rdr.deserialize() {
        let entry: RawKeylogEntry = row?;
        res.push(entry);
    }
    Ok(res)
}

pub fn parse_from_str(s: &str) -> Result<Vec<RawKeylogEntry>> {
    let cursor = Cursor::new(s);

    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(cursor);

    let mut res = Vec::new();
    for row in rdr.deserialize() {
        let entry: RawKeylogEntry = row?;
        res.push(entry);
    }
    Ok(res)
}
