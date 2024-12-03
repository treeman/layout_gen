mod keylog;
mod parse;
mod render;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use eyre::Result;
use parse::InputInfo;
use parse::Keymap;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[command(flatten)]
    keymap: KeymapArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args, Debug)]
struct KeymapArgs {
    #[arg(long)]
    qmk_root: String,

    #[arg(long)]
    keyboard: String,

    #[arg(long, default_value = "default")]
    keymap: String,

    #[arg(long, value_name = "RENDER_OPTS.json")]
    render_opts: String,
}

#[derive(Subcommand, Debug)]
enum Command {
    Render {
        #[arg(long)]
        output: String,
    },
    Stats {
        #[arg(long, value_name = "KEYLOG.CSV")]
        log: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    let info = InputInfo::parse(
        args.keymap.qmk_root.into(),
        args.keymap.keyboard,
        args.keymap.keymap,
        args.keymap.render_opts.into(),
    )?;

    match args.command {
        Command::Render { output } => render::render(&info, &Utf8PathBuf::from(output)),
        Command::Stats { log } => keylog::output_stats(&info, &Utf8PathBuf::from(log)),
    }
}
