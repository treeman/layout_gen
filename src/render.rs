use crate::parse::Combo;
use crate::parse::Key;
use crate::parse::Keymap;
use crate::parse::Layer;
use crate::render_opts::RenderOpts;
use camino::Utf8Path;
use eyre::Result;
use palette::Mix;
use palette::{Hsv, IntoColor, Srgb};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;

// TODO
// - Generate combos
// - Add wrapping class specifying keyboard/keymap name

pub fn render(keymap: &Keymap, render_opts: &RenderOpts, output_dir: &Utf8Path) -> Result<()> {
    for layer in keymap.layers.iter() {
        render_layer(layer, render_opts, output_dir)?;
    }

    render_legend(render_opts, output_dir)?;

    let base_layer = &keymap.layers[0];
    render_combos(&keymap.combos, base_layer, render_opts, output_dir)?;

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

        let x = keymap_border + col as f32 * key_w;
        let y = keymap_border + row as f32 * key_h;

        let outer_color = render_opts
            .colors
            .get(&item.class)
            .unwrap_or(&fallback_color);

        KeyRender {
            x,
            y,
            w: key_w,
            h: key_h,
            rx: 5.0,
            class,
            outer_color,
            title: txt,
            hold_title: None,
            border_left: 6.0,
            border_right: 6.0,
            border_top: 4.0,
            border_bottom: 8.0,
            text_h: 11.0,
        }
        .render(&mut file)?;
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

    write_layer_keys(
        &mut file,
        layer,
        render_opts,
        keymap_border,
        key_w,
        None,
        None,
        None,
    )?;

    file.write_all("</svg>".as_bytes())?;

    Ok(())
}

// TODO split out in strut
// TODO can render svg viewport as well
// TODO should be able to only render one half of the keyboard
#[allow(clippy::too_many_arguments)]
fn write_layer_keys(
    file: &mut File,
    layer: &Layer,
    render_opts: &RenderOpts,
    keymap_border: f32,
    key_w: f32,
    override_class: Option<&str>,
    override_class_map: Option<HashMap<&str, String>>,
    blank_class: Option<&str>,
) -> Result<()> {
    let fallback_color = "#e5c494".to_string();
    for key in layer.keys.iter() {
        let key_opts = render_opts.get(&layer.id.0, &key.id.0);
        let mut class = key_opts.class.as_str();
        if let Some(x) = override_class {
            class = x;
        }

        if let Some(override_map) = &override_class_map {
            if let Some(x) = override_map.get(key.id.0.as_str()) {
                class = x;
            }
        }
        let outer_color = render_opts.colors.get(class).unwrap_or(&fallback_color);

        let x = keymap_border + key.x * key_w;
        let y = keymap_border + key.y * key_w;
        let w = key_w;
        let h = key_w;

        let (title, hold_title) = if Some(class) == blank_class {
            ("", None)
        } else {
            (key_opts.title.as_str(), key_opts.hold_title.as_deref())
        };

        KeyRender {
            x,
            y,
            w,
            h,
            rx: 5.0,
            class,
            outer_color,
            title,
            hold_title,
            border_left: 6.0,
            border_right: 6.0,
            border_top: 4.0,
            border_bottom: 8.0,
            text_h: 11.0,
        }
        .render(file)?;
    }
    Ok(())
}

fn render_combos(
    combos: &[Combo],
    base_layer: &Layer,
    render_opts: &RenderOpts,
    output_dir: &Utf8Path,
) -> Result<()> {
    let mut mid_triple_combos = Vec::new();
    let mut neighbour_combos = Vec::new();
    let mut combos_with_separate_layouts = HashMap::new();
    let mut other_combos = Vec::new();

    for combo in combos {
        let mut handled = false;

        // A combo can be contained in several of the separate layouts
        for key in &combo.keys {
            let id = &key.id.0;
            if combo.keys.len() == 2 && render_opts.combos.keys_with_separate_imgs.contains(id) {
                combos_with_separate_layouts
                    .entry(id)
                    .and_modify(|key_combos: &mut Vec<&Combo>| key_combos.push(combo))
                    .or_insert_with(|| vec![combo]);
                handled = true;
            }
        }

        if !handled {
            if combo.is_mid_triple() {
                mid_triple_combos.push(combo);
            } else if combo.is_horizontal_neighbour() || combo.is_vertical_neighbour() {
                neighbour_combos.push(combo);
            } else {
                other_combos.push(combo);
            }
        }
    }

    CombosWithLayerRender {
        combos: &neighbour_combos,
        base_layer,
        render_opts,
        path: &output_dir.join("neighbour_combos.svg"),
    }
    .render()?;

    CombosWithLayerRender {
        combos: &mid_triple_combos,
        base_layer,
        render_opts,
        path: &output_dir.join("mid_triple_combos.svg"),
    }
    .render()?;

    for (active_key, combos) in &combos_with_separate_layouts {
        ComboSeparateLayerRender {
            active_key,
            combos,
            base_layer,
            render_opts,
            path: &output_dir.join(format!("{active_key}.svg")),
        }
        .render()?;
    }

    for combo in &other_combos {
        ComboSingleRender {
            combo,
            base_layer,
            render_opts,
            path: &output_dir.join(format!("{}.svg", combo.id)),
        }
        .render()?;
    }

    println!("Neighbours: {}", neighbour_combos.len());
    println!("Triple: {}", mid_triple_combos.len());
    for (key, combos) in &combos_with_separate_layouts {
        println!("{}: {}", key, combos.len());
    }
    println!("Other: {}", other_combos.len());
    println!("Total: {}", combos.len());

    Ok(())
}

struct CombosWithLayerRender<'a> {
    combos: &'a [&'a Combo],
    base_layer: &'a Layer,
    render_opts: &'a RenderOpts,
    path: &'a Utf8Path,
}

impl<'a> CombosWithLayerRender<'a> {
    fn render(&self) -> Result<()> {
        let mut file = File::create(self.path)?;

        let key_w = 54.0;
        let keymap_border = 10.0;
        let combo_text_h = 8.0;

        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        for key in self.base_layer.keys.iter() {
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

        writeln!(
            file,
            r#" <style type='text/css'>
    .keycap .border {{ stroke: black; stroke-width: 1; }}
    .keycap .inner.border {{ stroke: rgba(0,0,0,.1); }}
    .keycap {{ font-family: sans-serif; font-size: 11px}}
    .combos .keycap {{ font-size: {combo_text_h}px}}
  </style>
"#
        )?;

        write_layer_keys(
            &mut file,
            self.base_layer,
            self.render_opts,
            keymap_border,
            key_w,
            Some(self.render_opts.combos.background_layer_class.as_str()),
            None,
            None,
        )?;

        let fallback_color = "#e5c494".to_string();
        writeln!(file, r#"<g class="combos">"#)?;
        for combo in self.combos {
            let output_opts = self.render_opts.get(&self.base_layer.id.0, &combo.output);

            let title = &output_opts.title;
            let class = &output_opts.class;
            let outer_color = self
                .render_opts
                .colors
                .get(class)
                .unwrap_or(&fallback_color);

            ComboRender {
                combo,
                title,
                class,
                outer_color,
                keymap_border,
            }
            .render(&mut file)?;
        }

        writeln!(file, r#"</g>"#)?;

        file.write_all("</svg>".as_bytes())?;

        Ok(())
    }
}

struct ComboRender<'a> {
    combo: &'a Combo,
    title: &'a str,
    class: &'a str,
    outer_color: &'a str,
    keymap_border: f32,
}

impl<'a> ComboRender<'a> {
    fn render_key(&self, x: f32, y: f32, w: f32, h: f32, file: &mut File) -> Result<()> {
        let border_x = 1.5;
        let border_top = 1.0;
        let border_bottom = 2.5;

        let combo_text_h = 8.0;

        KeyRender {
            x,
            y,
            w,
            h,
            rx: 4.0,
            class: self.class,
            outer_color: self.outer_color,
            title: self.title,
            hold_title: None,
            border_left: border_x,
            border_right: border_x,
            border_top,
            border_bottom,
            text_h: combo_text_h,
        }
        .render(file)?;
        Ok(())
    }

    fn render(&self, file: &mut File) -> Result<()> {
        let key_w = 54.0;
        let combo_char_w = 5.0;
        let text_padding = 10.0;
        let combo_key_h = 16.0;

        let calc_w = |title: &str, min_w: f32| {
            let calc = title.chars().count() as f32 * combo_char_w + text_padding;
            calc.max(min_w)
        };

        if self.combo.is_vertical_neighbour() {
            let w = calc_w(self.title, 28.0);

            let a = &self.combo.keys[0];
            let b = &self.combo.keys[1];

            let x = self.keymap_border + a.x * key_w + key_w / 2.0 - w / 2.0;
            let y = self.keymap_border + (1.0 + a.y.min(b.y)) * key_w - combo_key_h / 2.0;

            self.render_key(x, y, w, combo_key_h, file)?;
        } else if self.combo.is_horizontal_neighbour() {
            let w = calc_w(self.title, 28.0);

            let a = &self.combo.keys[0];
            let b = &self.combo.keys[1];

            // The top y that intersects both keys
            let top_y_edge = a.y.max(b.y) * key_w;
            // The bottom y that intersects both keys
            let bottom_y_edge = a.y.min(b.y) * key_w + key_w;
            // Finds the middle of the intersection.
            let mid_y = top_y_edge + (bottom_y_edge - top_y_edge) / 2.0;
            // Offset with border and center the key at middle.
            let y = self.keymap_border + mid_y - combo_key_h / 2.0;
            // Right in the middle of the keys.
            let x = self.keymap_border + a.x.max(b.x) * key_w - w / 2.0;

            self.render_key(x, y, w, combo_key_h, file)?;
        } else if self.combo.is_mid_triple() {
            let w = calc_w(self.title, 80.0);

            let a = &self.combo.keys[0];
            let b = &self.combo.keys[1];
            let c = &self.combo.keys[2];

            // The top y that intersects both keys
            let top_y_edge = a.y.max(b.y).max(c.y) * key_w;
            // The bottom y that intersects both keys
            let bottom_y_edge = a.y.min(b.y).min(c.y) * key_w + key_w;
            // Finds the middle of the intersection.
            let mid_y = top_y_edge + (bottom_y_edge - top_y_edge) / 2.0;
            // Offset with border and center the key at middle.
            let y = self.keymap_border + mid_y - combo_key_h / 2.0;
            // Right in the middle of the keys.
            let x = self.keymap_border + (1.5 + a.x) * key_w - w / 2.0;

            self.render_key(x, y, w, combo_key_h, file)?;
        }
        Ok(())
    }
}

struct ComboSeparateLayerRender<'a> {
    active_key: &'a str,
    combos: &'a [&'a Combo],
    base_layer: &'a Layer,
    render_opts: &'a RenderOpts,
    path: &'a Utf8Path,
}

impl<'a> ComboSeparateLayerRender<'a> {
    fn render(&self) -> Result<()> {
        let mut layer = self.base_layer.clone();

        let mut class_overrides = HashMap::new();
        for combo in self.combos {
            let output_opts = self.render_opts.get(&self.base_layer.id.0, &combo.output);
            for key in &combo.keys {
                if key.id.0 == self.active_key {
                    continue;
                }
                layer.replace_key_id(&key.id.0, &combo.output);

                class_overrides.insert(combo.output.as_str(), output_opts.class.to_string());
            }
        }
        class_overrides.insert(
            self.active_key,
            self.render_opts
                .combos
                .active_class_in_separate_layer
                .clone(),
        );

        let mut file = File::create(self.path)?;

        let key_w = 54.0;
        let keymap_border = 10.0;
        let combo_text_h = 8.0;

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

        writeln!(
            file,
            r#" <style type='text/css'>
    .keycap .border {{ stroke: black; stroke-width: 1; }}
    .keycap .inner.border {{ stroke: rgba(0,0,0,.1); }}
    .keycap {{ font-family: sans-serif; font-size: 11px}}
    .combos .keycap {{ font-size: {combo_text_h}px}}
  </style>
"#
        )?;

        let background_layer_class = self.render_opts.combos.background_layer_class.as_str();

        write_layer_keys(
            &mut file,
            &layer,
            self.render_opts,
            keymap_border,
            key_w,
            Some(background_layer_class),
            Some(class_overrides),
            Some(background_layer_class),
        )?;

        writeln!(file, r"</svg>")?;

        Ok(())
    }
}

struct ComboSingleRender<'a> {
    combo: &'a Combo,
    base_layer: &'a Layer,
    render_opts: &'a RenderOpts,
    path: &'a Utf8Path,
}

impl<'a> ComboSingleRender<'a> {
    fn render(&self) -> Result<()> {
        let mut class_overrides = HashMap::new();
        let output_opts = self
            .render_opts
            .get(&self.base_layer.id.0, &self.combo.output);
        for key in &self.combo.keys {
            class_overrides.insert(key.id.0.as_str(), output_opts.class.to_string());
        }

        let mut file = File::create(self.path)?;

        let key_w = 54.0;
        let keymap_border = 10.0;
        let combo_text_h = 8.0;

        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        for key in self.base_layer.keys.iter() {
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

        writeln!(
            file,
            r#" <style type='text/css'>
    .keycap .border {{ stroke: black; stroke-width: 1; }}
    .keycap .inner.border {{ stroke: rgba(0,0,0,.1); }}
    .keycap {{ font-family: sans-serif; font-size: 11px}}
    .combo-output {{ font-family: sans-serif; font-size: 16px}}
    .combos .keycap {{ font-size: {combo_text_h}px}}
  </style>
"#
        )?;

        let background_layer_class = self.render_opts.combos.background_layer_class.as_str();

        write_layer_keys(
            &mut file,
            self.base_layer,
            self.render_opts,
            keymap_border,
            key_w,
            Some(background_layer_class),
            Some(class_overrides),
            None,
        )?;

        let text_x = max_x / 2.0;
        let text_y = 12.0;
        let text = &self.combo.output;

        writeln!(
            file,
            r#"<text x="{text_x}" y="{text_y}" text-anchor="middle" dominant-baseline="middle" class="combo-output">{text}</text>"#
        )?;

        writeln!(file, r"</svg>")?;

        Ok(())
    }
}

struct KeyRender<'a> {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rx: f32,
    class: &'a str,
    outer_color: &'a str,
    title: &'a str,
    text_h: f32,
    hold_title: Option<&'a str>,
    border_left: f32,
    border_right: f32,
    border_top: f32,
    border_bottom: f32,
}

impl<'a> KeyRender<'a> {
    fn render(&self, file: &mut File) -> Result<()> {
        let outer_x = self.x;
        let outer_y = self.y;
        let outer_w = self.w;
        let outer_h = self.h;

        let inner_w = outer_w - (self.border_left + self.border_right);
        let inner_h = outer_h - (self.border_top + self.border_bottom);

        let inner_x = outer_x + self.border_left;
        let inner_y = outer_y + self.border_top;

        let outer_color = self.outer_color;
        let inner_color = lighten_color(Srgb::from_str(outer_color).unwrap().into(), 0.1);
        let inner_color = format!("#{:x}", Srgb::<u8>::from(inner_color));

        let class = self.class;
        let rx = self.rx;

        writeln!(
            file,
            r##"    <g class="keycap {class}">
      <rect x="{outer_x}" y="{outer_y}"
            width="{outer_w}" height="{outer_h}"
            rx="{rx}" fill="{outer_color}" class="outer border"/>
      <rect x="{inner_x}" y="{inner_y}"
            width="{inner_w}" height="{inner_h}"
            rx="{rx}" fill="{inner_color}" class="inner border"/>
"##,
        )?;

        let text = self.title.lines().collect::<Vec<_>>();
        if !text.is_empty() {
            let y_offset = (text.len() - 1) as f32 * self.text_h / 2.0;
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
                    _ => self.text_h,
                };
                writeln!(file, r#"<tspan x="{text_x}" dy="{dy}">{txt}</tspan>"#)?;
            }

            writeln!(file, "</text>")?;
        }

        if let Some(subtxt) = self.hold_title {
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
