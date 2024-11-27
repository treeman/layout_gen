mod parse;
mod render;
mod render_opts;

use camino::Utf8PathBuf;
use clap::Parser;
use eyre::Result;
use parse::Keymap;
use parse::ParseSettings;
use render_opts::RenderOpts;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[arg(long)]
    qmk_root: String,

    #[arg(long)]
    keyboard: String,

    #[arg(long, default_value = "default")]
    keymap: String,

    #[arg(long)]
    render_opts: String,

    #[arg(long)]
    output: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let render_opts = RenderOpts::parse(&Utf8PathBuf::from(args.render_opts))?;

    let keymap = Keymap::parse(
        &ParseSettings {
            qmk_root: args.qmk_root.into(),
            keyboard: args.keyboard,
            keymap: args.keymap,
        },
        &render_opts,
    )?;

    let output_dir = Utf8PathBuf::from(args.output);
    render::render(&keymap, &render_opts, &output_dir)?;

    Ok(())
}
