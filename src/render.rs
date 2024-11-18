use crate::parse::Keymap;
use crate::parse::Layer;
use camino::Utf8Path;
use eyre::Result;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::sync::LazyLock;

pub fn render(keymap: &Keymap, render_opts: &Utf8Path, output_dir: &Utf8Path) -> Result<()> {
    let render_opts = RenderOpts::parse(render_opts)?;

    for layer in keymap.layers.iter() {
        render_layer(layer, &render_opts, output_dir)?;
    }

    Ok(())
}

fn render_layer(layer: &Layer, render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let path = output_dir.join(format!("{}.svg", layer.id.0));
    let mut file = File::create(path)?;

    // TODO
    // - Calcualate viewbox better from keys
    // - Need to have a way to deal with keys that activate the current layer
    // - Customize font size?

    file.write_all(
        r#"<svg width='800px'
       height='320px'
       viewBox='0 0 800 320'
       xmlns='http://www.w3.org/2000/svg'
       xmlns:xlink="http://www.w3.org/1999/xlink">
"#
        .as_bytes(),
    )?;

    file.write_all(
        r#" <style type='text/css'>
    .keycap .border { stroke: black; stroke-width: 1; }
    .keycap .inner.border { stroke: rgba(0,0,0,.1); }
    .keycap { font-family: sans-serif; font-size: 11px}
  </style>
"#
        .as_bytes(),
    )?;

    for key in layer.keys.iter() {
        let keyboard_border = 10.0;
        let border_w = 6.0;
        let border_top = 4.0;

        let inner_w = 40.0;
        let outer_w = inner_w + border_w * 2.0;

        let outer_x = keyboard_border + key.x * outer_w;
        let outer_y = keyboard_border + key.y * outer_w;

        let inner_x = keyboard_border + key.x * outer_w + border_w;
        let inner_y = keyboard_border + key.y * outer_w + border_top;

        let key_opts = render_opts.get(&layer.id.0, &key.id.0);

        let class = key_opts.class;

        writeln!(
            file,
            r##"    <g class="keycap {class}">
      <rect x="{outer_x}" y="{outer_y}"
            width="{outer_w}" height="{outer_w}"
            rx="5" fill="#e5c494" class="outer border"/>
      <rect x="{inner_x}" y="{inner_y}"
            width="{inner_w}" height="{inner_w}"
            rx="5" fill="#fff3c1" class="inner border"/>
"##,
        )?;

        let text_h = 12.0;

        let text = key_opts.title.lines().collect::<Vec<_>>();

        if !text.is_empty() {
            // let total_h = text.len() as f32 * text_h;
            let y_offset = (text.len() - 1) as f32 * text_h / 2.0;

            let text_x = inner_x + inner_w / 2.0;
            let text_y = inner_y + inner_w / 2.0 - y_offset;

            writeln!(
                file,
                r#"<text x="{text_x}" y="{text_y}" text-anchor="middle" dominant-baseline="middle">"#
            )?;

            for (i, txt) in text.into_iter().enumerate() {
                let txt = html_escape::encode_safe(&txt);
                let dy = match i {
                    0 => 0.0,
                    _ => text_h,
                };
                writeln!(file, r#"<tspan x="{text_x}" dy="{dy}">{txt}</tspan>"#)?;
            }

            writeln!(file, "</text>")?;
        }
        writeln!(file, "</g>")?;
    }

    file.write_all("</svg>".as_bytes())?;

    Ok(())
}

#[derive(Debug)]
struct RenderOpts {
    default_keys: HashMap<String, PartialKeyOpts>,
    layer_keys: HashMap<String, HashMap<String, PartialKeyOpts>>,
}

impl RenderOpts {
    fn parse(file: &Utf8Path) -> Result<Self> {
        let src = fs::read_to_string(file)?;
        Self::parse_from_str(&src)
    }

    fn parse_from_str(s: &str) -> Result<Self> {
        let spec: RenderSpec = serde_json::from_str(s)?;
        Ok(Self::new(spec))
    }

    fn new(spec: RenderSpec) -> Self {
        let mut default_keys = HashMap::new();
        let mut layer_keys: HashMap<String, HashMap<String, PartialKeyOpts>> = HashMap::new();

        for (layer_id, layer) in spec {
            for key_spec in &layer {
                for key in &key_spec.keys {
                    let opts = PartialKeyOpts::from_spec(key, key_spec);

                    if layer_id == "default" {
                        default_keys.insert(key.to_owned(), opts);
                    } else {
                        layer_keys
                            .entry(layer_id.to_string())
                            .or_default()
                            .insert(key.to_owned(), opts);
                    }
                }
            }
        }
        Self {
            default_keys,
            layer_keys,
        }
    }

    pub fn get(&self, layer_id: &str, key_id: &str) -> KeyOpts {
        let mut res = KeyOpts::with_defaults(key_id);

        if let Some(opts) = self.default_keys.get(key_id) {
            res.merge(opts);
        }

        if let Some(keys) = self.layer_keys.get(layer_id) {
            if let Some(opts) = keys.get(key_id) {
                res.merge(opts);
            }
        }
        res
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyOpts {
    id: String,
    title: String,
    hold_title: Option<String>,
    class: String,
}

impl KeyOpts {
    fn with_defaults(key_id: &str) -> Self {
        Self {
            id: key_id.to_string(),
            title: key_id_to_title(key_id),
            hold_title: None,
            class: "default".to_string(),
        }
    }

    fn merge(&mut self, opts: &PartialKeyOpts) -> &mut Self {
        assert_eq!(self.id, opts.id);
        if let Some(ref title) = opts.title {
            self.title = title.to_owned();
        }
        if let Some(ref hold_title) = opts.hold_title {
            self.hold_title = Some(hold_title.to_owned());
        }
        if let Some(ref class) = opts.class {
            self.class = class.to_owned();
        }
        self
    }
}

fn key_id_to_title(id: &str) -> String {
    static BASIC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(SE|KC)_([\w\d]|\d+|F\d+)$").unwrap());
    if let Some(basic) = BASIC.captures(id) {
        return basic[2].to_string();
    }
    let res = match id {
        "SE_DOT" => ".",
        "SE_COMM" => ",",
        "SE_SLSH" => "/",
        "SE_LPRN" => "(",
        "SE_RPRN" => ")",
        "SE_UNDS" => "_",
        "SE_TILD" => "~",
        "TILD" => "~",
        "SE_PLUS" => "+",
        "SE_ASTR" => "*",
        "SE_EXLM" => "!",
        "SE_PIPE" => "|",
        "SE_HASH" => "#",
        "SE_AT" => "@",
        "SE_CIRC" => "^",
        "CIRC" => "^",
        "SE_LCBR" => "{",
        "SE_RCBR" => "}",
        "SE_MINS" => "-",
        "SE_BSLS" => "\\",
        "SE_GRV" => "`",
        "GRV" => "`",
        "SE_QUES" => "?",
        "SE_LBRC" => "[",
        "SE_RBRC" => "]",
        "SE_LABK" => "<",
        "SE_RABK" => ">",
        "SE_PERC" => "%",
        "SE_AMPR" => "&",
        "SE_ARNG" => "Å",
        "SE_ADIA" => "Ä",
        "SE_ODIA" => "Ö",
        "SE_ACUT" => "´",
        "SE_DIAE" => "¨",
        "KC_UP" => "↑",
        "KC_DOWN" => "↓",
        "KC_LEFT" => "←",
        "KC_RGHT" => "→",
        "KC_HOME" => "Home",
        "KC_END" => "End",
        "KC_PGUP" => "PgUp",
        "KC_PGDN" => "PgDn",
        _ => id,
    };
    res.to_string()
}

#[derive(Debug, Clone)]
struct PartialKeyOpts {
    id: String,
    title: Option<String>,
    hold_title: Option<String>,
    class: Option<String>,
}

impl PartialKeyOpts {
    fn from_spec(key_id: &str, spec: &KeySpec) -> Self {
        Self {
            id: key_id.to_string(),
            title: spec.title.clone(),
            hold_title: spec.hold_title.clone(),
            class: spec.class.clone(),
        }
    }
}

type RenderSpec = HashMap<String, LayerSpec>;
type LayerSpec = Vec<KeySpec>;

#[derive(Deserialize, Debug)]
struct KeySpec {
    keys: Vec<String>,
    title: Option<String>,
    hold_title: Option<String>,
    class: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::Result;

    #[test]
    fn test_parse_render_opts() -> Result<()> {
        let input = r#"
{
  "default": [
    {
      "keys": ["_______", "xxxxxxx"],
      "title": "",
      "class": "blank"
    },
    {
      "keys": ["SE_LPRN"],
      "title": "("
    }
  ],
  "_NUM": [
    {
      "keys": ["SE_J", "SE_P", "SE_K", "AT_U", "SE_LPRN", "SE_RPRN", "NUM_G"],
      "class": "management"
    }
  ]
}
        "#;
        let opts = RenderOpts::parse_from_str(input)?;

        let a = opts.get("_BASE", "SE_A");
        assert_eq!(
            a,
            KeyOpts {
                id: "SE_A".to_string(),
                title: "A".to_string(),
                hold_title: None,
                class: "default".to_string(),
            }
        );

        let lprn = opts.get("_NUM", "SE_LPRN");
        assert_eq!(
            lprn,
            KeyOpts {
                id: "SE_LPRN".to_string(),
                title: "(".to_string(),
                hold_title: None,
                class: "management".to_string(),
            }
        );

        Ok(())
    }
}
