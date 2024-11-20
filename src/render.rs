use crate::parse::Combo;
use crate::parse::Key;
use crate::parse::Keymap;
use crate::parse::Layer;
use crate::render_opts::RenderOpts;
use camino::Utf8Path;
use eyre::Result;
use palette::{Hsv, IntoColor, Srgb};
use std::fs::File;
use std::io::Write;
use std::str::FromStr;

// TODO
// - Generate combos
// - Set default colors for different types
// - Add wrapping class specifying keyboard/keymap name

pub fn render(keymap: &Keymap, render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    for layer in keymap.layers.iter() {
        render_layer(layer, render_opts, output_dir)?;
    }

    render_legend(render_opts, output_dir)?;

    render_combos(&keymap.combos, render_opts, output_dir)?;

    Ok(())
}

fn render_legend(render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let path = output_dir.join("legend.svg");
    let mut file = File::create(path)?;

    let keymap_border = 10.0;
    let key_side = 54.0;
    let key_w = 4.0 * key_side;
    let key_h = key_side;

    let item_count = render_opts.legend.len();
    let columns = std::cmp::min(item_count, 4);
    let rows = item_count / columns;

    let max_x = columns as f32 * key_w + keymap_border * 2.0;
    let max_y = rows as f32 * key_h + keymap_border * 2.0;

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
        let txt = &item.title;

        let outer_x = keymap_border + col as f32 * key_w;
        let outer_y = keymap_border + row as f32 * key_h;

        let outer_color = render_opts
            .colors
            .get(&item.class)
            .unwrap_or(&fallback_color);

        write_bordered_key(
            &mut file,
            outer_x,
            outer_y,
            key_w,
            key_h,
            class,
            outer_color,
            txt,
            None,
        )?;
    }

    file.write_all("</svg>".as_bytes())?;

    Ok(())
}

fn render_layer(layer: &Layer, render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let path = output_dir.join(format!("{}.svg", layer.id.0));
    let mut file = File::create(path)?;

    let key_w = 54.0;
    let keymap_border = 10.0;

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    for key in layer.keys.iter() {
        max_x = max_x.max((1.0 + key.x) * key_w);
        max_y = max_y.max((1.0 + key.y) * key_w);
    }
    max_x += keymap_border * 2.0;
    max_y += keymap_border * 2.0;

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

    write_layer_keys(&mut file, layer, render_opts, keymap_border, key_w)?;

    file.write_all("</svg>".as_bytes())?;

    Ok(())
}

fn write_layer_keys(
    file: &mut File,
    layer: &Layer,
    render_opts: &RenderOpts,
    keymap_border: f32,
    key_w: f32,
) -> Result<()> {
    let fallback_color = "#e5c494".to_string();
    for key in layer.keys.iter() {
        let key_opts = render_opts.get(&layer.id.0, &key.id.0);
        let class = key_opts.class;
        let outer_color = render_opts.colors.get(&class).unwrap_or(&fallback_color);

        let outer_x = keymap_border + key.x * key_w;
        let outer_y = keymap_border + key.y * key_w;

        write_bordered_key(
            file,
            outer_x,
            outer_y,
            key_w,
            key_w,
            &class,
            outer_color,
            &key_opts.title,
            key_opts.hold_title.as_deref(),
        )?;
    }
    Ok(())
}

fn render_combos(combos: &[Combo], render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    let mut horizontal_combos = Vec::new();
    let mut vertical_combos = Vec::new();

    // TODO don't hardcode these
    let mut e_combos = Vec::new();
    let mut spc_combos = Vec::new();
    let mut fun_combos = Vec::new();

    let mut other_combos = Vec::new();

    for combo in combos {
        if combo.is_horizontal_neighbour() {
            horizontal_combos.push(combo);
        } else if combo.is_vertical_neighbour() {
            vertical_combos.push(combo);
            // FIXME can be both E and SPC
        } else if combo.contains_input_key("SE_E") && combo.keys.len() == 2 {
            e_combos.push(combo);
        } else if combo.contains_input_key("MT_SPC") && combo.keys.len() == 2 {
            spc_combos.push(combo);
        } else if combo.contains_input_key("FUN") && combo.keys.len() == 2 {
            fun_combos.push(combo);
        } else {
            other_combos.push(combo);
        }
    }

    println!(
        "hor: {} ver: {} e: {} spc: {} fun: {} other: {}",
        horizontal_combos.len(),
        vertical_combos.len(),
        e_combos.len(),
        spc_combos.len(),
        fun_combos.len(),
        other_combos.len()
    );

    for combo in other_combos {
        for x in &combo.keys {
            print!(" {}", x.id.0);
        }
        println!();
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_bordered_key(
    file: &mut File,
    outer_x: f32,
    outer_y: f32,
    outer_w: f32,
    outer_h: f32,
    class: &str,
    outer_color: &str,
    title: &str,
    hold_title: Option<&str>,
) -> Result<()> {
    let border_w = 6.0;
    let border_top = 4.0;

    let inner_w = outer_w - border_w * 2.0;
    let inner_h = outer_h - border_w * 2.0;

    let inner_x = outer_x + border_w;
    let inner_y = outer_y + border_top;

    let inner_color = lighten_color(Srgb::from_str(outer_color).unwrap().into(), 0.1);
    let inner_color = format!("#{:x}", Srgb::<u8>::from(inner_color));

    writeln!(
        file,
        r##"    <g class="keycap {class}">
      <rect x="{outer_x}" y="{outer_y}"
            width="{outer_w}" height="{outer_h}"
            rx="5" fill="{outer_color}" class="outer border"/>
      <rect x="{inner_x}" y="{inner_y}"
            width="{inner_w}" height="{inner_h}"
            rx="5" fill="{inner_color}" class="inner border"/>
"##,
    )?;

    let text_h = 12.0;

    let text = title.lines().collect::<Vec<_>>();
    if !text.is_empty() {
        let y_offset = (text.len() - 1) as f32 * text_h / 2.0;
        let text_x = inner_x + inner_w / 2.0;
        let text_y = inner_y + inner_h / 2.0 - y_offset;

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

    if let Some(subtxt) = hold_title {
        let text_x = inner_x + inner_w / 2.0;
        let text_y = inner_y + inner_w + 6.2;

        writeln!(
            file,
            r#"<text x="{text_x}" y="{text_y}" text-anchor="middle" class="sub">{subtxt}</text>"#
        )?;
    }
    writeln!(file, "</g>")?;

    Ok(())
}

fn lighten_color(rgb: Srgb, amount: f32) -> Srgb {
    // Convert RGB to HSV
    let hsv: Hsv = rgb.into_color();

    // Increase the lightness
    let new_value = (hsv.value + amount).min(1.0); // Ensure it doesn't exceed 1.0
    let new_hsv = Hsv::new(hsv.hue, hsv.saturation, new_value);

    // Convert back to RGB
    new_hsv.into_color()
}
