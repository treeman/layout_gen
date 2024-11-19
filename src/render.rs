use crate::parse::Keymap;
use crate::parse::Layer;
use camino::Utf8Path;
use eyre::Result;
use palette::{Hsl, IntoColor, Srgb};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::sync::LazyLock;

// TODO
// - Generate legend
// - Generate combos
// - Set default colors for different types
// - Add wrapping class specifying keyboard/keymap name

pub fn render(keymap: &Keymap, render_opts: &Utf8Path, output_dir: &Utf8Path) -> Result<()> {
    let render_opts = RenderOpts::parse(render_opts)?;

    for layer in keymap.layers.iter() {
        render_layer(layer, &render_opts, output_dir)?;
    }

    render_legend(&render_opts, output_dir)?;

    Ok(())
}

fn render_legend(render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let path = output_dir.join("legend.svg");
    let mut file = File::create(path)?;

    let border = 10.0;
    let border_w = 6.0;
    let border_top = 4.0;
    let border_bottom = border_w * 2.0 - border_top;

    let inner_h = 42.0;
    let outer_h = inner_h + border_w * 2.0;

    let inner_w = 4.0 * 42.0;
    let outer_w = inner_w + border_w * 2.0;

    let item_count = render_opts.legend.len();
    let columns = std::cmp::min(item_count, 4);
    let rows = item_count / columns;

    let max_x = columns as f32 * outer_w + border * 2.0;
    let max_y = rows as f32 * outer_h + border * 2.0;

    writeln!(
        file,
        r#"<svg width='{max_x}px'
    height='{max_y}x'
    viewBox='0 0 {max_x} {max_y}'
    xmlns='http://www.w3.org/2000/svg'
    xmlns:xlink="http://www.w3.org/1999/xlink">
"#
    )?;

    file.write_all(
        r#" <style type='text/css'>
    .legend .border { stroke: black; stroke-width: 1; }
    .legend .inner.border { stroke: rgba(0,0,0,.1); }
    .legend { font-family: sans-serif; font-size: 11px}
  </style>
"#
        .as_bytes(),
    )?;

    let fallback_color = "#e5c494".to_string();
    for (i, item) in render_opts.legend.iter().enumerate() {
        let row = i / columns;
        let col = i - row * columns;

        let class = &item.class;
        let txt = html_escape::encode_safe(&item.title);

        let outer_x = border + col as f32 * outer_w;
        let outer_y = border + row as f32 * outer_h;

        let inner_x = border + col as f32 * outer_w + border_w;
        let inner_y = border + row as f32 * outer_h + border_top;

        let text_x = inner_x + inner_w / 2.0;
        let text_y = inner_y + inner_h / 2.0;

        let outer_color = render_opts
            .colors
            .get(&item.class)
            .unwrap_or(&fallback_color);

        let inner_color = lighten_color(Srgb::from_str(outer_color).unwrap().into(), 0.05);
        let inner_color = format!("#{:x}", Srgb::<u8>::from(inner_color));

        writeln!(
            file,
            r##"<g class="legend {class}">

      <rect x="{outer_x}" y="{outer_y}"
            width="{outer_w}" height="{outer_h}"
            rx="5" fill="{outer_color}" class="outer border"/>
      <rect x="{inner_x}" y="{inner_y}"
            width="{inner_w}" height="{inner_h}"
            rx="5" fill="{inner_color}" class="inner border"/>
    <text x="{text_x}" y="{text_y}" text-anchor="middle" dominant-baseline="middle">{txt}</text>
    </g>
"##,
        )?;
    }

    file.write_all("</svg>".as_bytes())?;

    Ok(())
}

fn render_layer(layer: &Layer, render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let path = output_dir.join(format!("{}.svg", layer.id.0));
    let mut file = File::create(path)?;

    // TODO
    // - Calcualate viewbox better from keys
    // - Need to have a way to deal with keys that activate the current layer
    // - Customize font size?

    let keyboard_border = 10.0;
    let border_w = 6.0;
    let border_top = 4.0;

    let inner_w = 42.0;
    let outer_w = inner_w + border_w * 2.0;

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    for key in layer.keys.iter() {
        max_x = max_x.max((1.0 + key.x) * outer_w);
        max_y = max_y.max((1.0 + key.y) * outer_w);
    }
    max_x += keyboard_border * 2.0;
    max_y += keyboard_border * 2.0;

    writeln!(
        file,
        r#"<svg width='{max_x}px'
       height='{max_y}x'
       viewBox='0 0 {max_x} {max_y}'
       xmlns='http://www.w3.org/2000/svg'
       xmlns:xlink="http://www.w3.org/1999/xlink">
"#
    )?;

    file.write_all(
        r#" <style type='text/css'>
    .keycap .border { stroke: black; stroke-width: 1; }
    .keycap .inner.border { stroke: rgba(0,0,0,.1); }
    .keycap { font-family: sans-serif; font-size: 11px}
    .keycap .sub { font-size: 9px}
  </style>
"#
        .as_bytes(),
    )?;

    let fallback_color = "#e5c494".to_string();
    for key in layer.keys.iter() {
        let outer_x = keyboard_border + key.x * outer_w;
        let outer_y = keyboard_border + key.y * outer_w;

        let inner_x = keyboard_border + key.x * outer_w + border_w;
        let inner_y = keyboard_border + key.y * outer_w + border_top;

        let key_opts = render_opts.get(&layer.id.0, &key.id.0);

        let class = key_opts.class;

        let outer_color = render_opts.colors.get(&class).unwrap_or(&fallback_color);

        let inner_color = lighten_color(Srgb::from_str(outer_color).unwrap().into(), 0.1);
        let inner_color = format!("#{:x}", Srgb::<u8>::from(inner_color));

        writeln!(
            file,
            r##"    <g class="keycap {class}">
      <rect x="{outer_x}" y="{outer_y}"
            width="{outer_w}" height="{outer_w}"
            rx="5" fill="{outer_color}" class="outer border"/>
      <rect x="{inner_x}" y="{inner_y}"
            width="{inner_w}" height="{inner_w}"
            rx="5" fill="{inner_color}" class="inner border"/>
"##,
        )?;

        let text_h = 12.0;

        let text = key_opts.title.lines().collect::<Vec<_>>();

        if !text.is_empty() {
            let y_offset = (text.len() - 1) as f32 * text_h / 2.0;

            let text_x = inner_x + inner_w / 2.0;
            let text_y = inner_y + inner_w / 2.0 - y_offset;

            writeln!(
                file,
                r#"<text x="{text_x}" y="{text_y}" text-anchor="middle" dominant-baseline="middle" class="main">"#
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

        if let Some(subtxt) = key_opts.hold_title {
            let text_x = inner_x + inner_w / 2.0;
            let text_y = inner_y + inner_w + 6.2;

            writeln!(
                file,
                r#"<text x="{text_x}" y="{text_y}" text-anchor="middle" class="sub">{subtxt}</text>"#
            )?;
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
    legend: Vec<LegendSpec>,
    colors: HashMap<String, String>,
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

        for (layer_id, layer) in spec.layers {
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
            legend: spec.legend,
            colors: spec.colors,
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

#[derive(Deserialize, Debug)]
struct RenderSpec {
    layers: LayersSpec,
    legend: Vec<LegendSpec>,
    colors: HashMap<String, String>,
}

type LayersSpec = HashMap<String, LayerSpec>;
type LayerSpec = Vec<KeySpec>;

#[derive(Deserialize, Debug)]
struct KeySpec {
    keys: Vec<String>,
    title: Option<String>,
    hold_title: Option<String>,
    class: Option<String>,
}

#[derive(Deserialize, Debug)]
struct LegendSpec {
    class: String,
    title: String,
}

fn lighten_color(rgb: Srgb, amount: f32) -> Srgb {
    // Convert RGB to HSL
    let hsl: Hsl = rgb.into_color();

    // Increase the lightness
    let new_lightness = (hsl.lightness + amount).min(1.0); // Ensure it doesn't exceed 1.0
    let new_hsl = Hsl::new(hsl.hue, hsl.saturation, new_lightness);

    // Convert back to RGB
    new_hsl.into_color()
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
