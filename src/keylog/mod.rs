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
