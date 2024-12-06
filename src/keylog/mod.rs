mod csv_parser;
mod stats;

use stats::*;

use crate::parse::InputInfo;
use camino::Utf8Path;
use eyre::Result;

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
        let perc = (*freq) as f32 / stats.total_key_presses as f32 * 100.0;
        stats_row.push_str(&format!("{perc:>7.2}%"));
    }
    println!();
    println!("{}", finger_row);
    println!("{}", stats_row);
    println!();

    let left = stats.total_key_presses_left as f32 / stats.total_key_presses as f32 * 100.0;
    println!("    left: {left:>7.2}%");
    let right = stats.total_key_presses_right as f32 / stats.total_key_presses as f32 * 100.0;
    println!("   right: {right:>7.2}%");

    output_sfbs(&stats, "sfbs (without combos)", false);
    output_sfbs(&stats, "sfbs (with combos)", true);

    Ok(())
}

fn output_sfbs(stats: &KeylogStats, title: &str, include_combos: bool) {
    let mut finger_row = String::new();
    let mut stats_row = String::new();
    for (finger, presses) in &stats.sfb_frequency_by_finger(include_combos) {
        finger_row.push_str(&format!("{:>8}", finger.finger.to_string()));
        let perc = *presses as f32 / stats.total_events as f32 * 100.0;
        stats_row.push_str(&format!("{perc:>7.2}%"));
    }
    println!();
    println!();
    println!("  {title}");
    println!("{}", finger_row);
    println!("{}", stats_row);
    println!();
    let perc = stats.sfb_perc(include_combos);
    println!("  total: {perc:>7.3}%",);

    println!("  top sfbs:");
    for sfb in stats.top_sfbs(10, include_combos) {
        let perc = sfb.presses as f32 / stats.total_events as f32 * 100.0;
        println!("   {:<35}     {perc:>.2}%", sfb.sfb.id());
    }

    println!();
    println!("  top sfbs by key:");
    for (id, freq) in stats.top_sfbs_by_key(10, include_combos) {
        let perc = freq as f32 / stats.total_events as f32 * 100.0;
        println!("   {:<35}     {perc:>.2}%", id);
    }
}
