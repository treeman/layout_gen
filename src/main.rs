mod parse;
mod render;

use camino::Utf8PathBuf;
use clap::Parser;
use eyre::Result;
use parse::Keymap;

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

#[derive(Debug)]
pub struct InputSettings {
    pub qmk_root: Utf8PathBuf,
    pub keyboard: String,
    pub keymap: String,
}

impl InputSettings {
    pub fn combos_def(&self) -> Utf8PathBuf {
        self.keymap_dir().join("combos.def")
    }

    pub fn keymap_c(&self) -> Utf8PathBuf {
        self.keymap_dir().join("keymap.c")
    }

    pub fn keyboard_json(&self) -> Utf8PathBuf {
        self.keyboard_dir().join("keyboard.json")
    }

    pub fn keyboard_dir(&self) -> Utf8PathBuf {
        self.qmk_root.join("keyboards").join(&self.keyboard)
    }

    pub fn keymap_dir(&self) -> Utf8PathBuf {
        self.keyboard_dir().join("keymaps").join(&self.keymap)
    }
}

fn generate() -> Result<()> {
    let args = Args::parse();
    let input = InputSettings {
        qmk_root: args.qmk_root.into(),
        keyboard: args.keyboard,
        keymap: args.keymap,
    };

    let keymap = Keymap::parse(&input)?;
    let output_dir = Utf8PathBuf::from(args.output);
    let render_opts = Utf8PathBuf::from(args.render_opts);

    render::render(&keymap, &render_opts, &output_dir)?;

    Ok(())
}

fn main() {
    generate().unwrap();
}
