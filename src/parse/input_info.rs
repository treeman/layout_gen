use camino::Utf8PathBuf;
use eyre::Result;

use super::keymap::{Keymap, ParseSettings};
use super::render_opts::RenderOpts;

#[derive(Debug, Clone)]
pub struct InputInfo {
    pub keymap: Keymap,
    pub render_opts: RenderOpts,
}

impl InputInfo {
    pub fn parse(
        qmk_root: Utf8PathBuf,
        keyboard: String,
        keymap: String,
        render_opts: Utf8PathBuf,
    ) -> Result<Self> {
        let render_opts = RenderOpts::parse(&render_opts)?;

        let keymap = Keymap::parse(
            &ParseSettings {
                qmk_root,
                keyboard,
                keymap,
            },
            &render_opts,
        )?;

        Ok(Self {
            keymap,
            render_opts,
        })
    }
}
