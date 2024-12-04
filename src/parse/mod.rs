#![allow(dead_code)]

mod data;
mod keymap;
mod render_opts;

pub use data::InputInfo;
pub use keymap::{Combo, Key, KeyId, Keymap, Layer, LayerId};
pub use render_opts::{Finger, FingerAssignment, MatrixHalf, RenderOpts};
