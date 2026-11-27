//! A colour picker and a swatch button that opens one (M25, D0030).
//!
//! The colour area and the tracks of the hue and opacity sliders are small
//! images, drawn again only when what they show changes, and stretched.

use dovetail::{
  AccessAction, Align, Border, Callback, Color, Edges, Element, ElementKind,
  KeyCode, KeyHandler, Layer, NamedKey, PointerHandler, PointerKind, Role,
  Size, Style, ValueRange, View, use_state,
};
use dovetail_image::{Bitmap, Rgba};
use dovetail_macros::{component, ui};
use dovetail_theme::color::{Hsv, parse_hex, to_byte, to_hex};
use dovetail_theme::theme;

use crate::{NumberField, TextField};

/// Pixels across the colour area's image.
const AREA_PIXELS: u32 = 64;
/// The colour area's size on screen.
const AREA: (f32, f32) = (220.0, 150.0);
/// A slider track's height.
const TRACK: f32 = 14.0;
/// The ring round the chosen point and the handles.
const RING: f32 = 14.0;

/// Pixels from colours, row by row, as an image to stretch.
fn image(
  width: u32,
  height: u32,
  colour: impl Fn(u32, u32) -> Color,
) -> Bitmap {
  let mut pixels = Vec::with_capacity((width * height * 4) as usize);
  for y in 0..height {
    for x in 0..width {
      let c = colour(x, y);
      pixels.extend([
        to_byte(c.r),
        to_byte(c.g),
        to_byte(c.b),
        to_byte(c.a),
      ]);
    }
  }
  Bitmap::from_rgba(Rgba { width, height, pixels })
}

/// `top` over `under`, both straight sRGB.
fn over(
  top: Color,
  under: Color,
) -> Color {
  let a = top.a;
  Color {
    r: top.r * a + under.r * (1.0 - a),
    g: top.g * a + under.g * (1.0 - a),
    b: top.b * a + under.b * (1.0 - a),
    a: 1.0,
  }
}

/// The squares behind a colour that is not solid, from the theme.
fn checks(
  x: u32,
  y: u32,
) -> Color {
  let (_, light, dark) = theme().colour_picker_look();
  if (x / 4 + y / 4).is_multiple_of(2) {
    light
  } else {
    dark
  }
}

/// `hsv`, with hue `h` when it is a grey, which has no hue of its own.
fn keep_hue(
  hsv: Hsv,
  h: f32,
) -> Hsv {
  if hsv.s <= 0.0 || hsv.v <= 0.0 {
    Hsv { h, ..hsv }
  } else {
    hsv
  }
}

/// Images that are kept while what they show stays the same.
#[derive(Clone, Default)]
struct Images {
  /// The colour area, for its hue.
  area: Option<(u32, Bitmap)>,
  /// The hue track.
  hue: Option<Bitmap>,
  /// The opacity track, for its colour.
  alpha: Option<(String, Bitmap)>,
}

/// A ring at `x`, `y` in its parent, as the chosen point or a handle.
fn ring(
  x: f32,
  y: f32,
  fill: Color,
) -> View {
  let (ink, ..) = theme().colour_picker_look();
  Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(RING),
      height: Size::Fixed(RING),
      radius: RING / 2.0,
      background: fill,
      border: Border { width: 2.0, color: ink },
      offset: (x - RING / 2.0, y - RING / 2.0),
      ..Style::default()
    })
    .into()
}

/// A slider drawn on an image, from 0 to `max`: the hue or the opacity.
#[allow(clippy::too_many_arguments)]
fn track(
  picture: &Bitmap,
  value: f32,
  max: f32,
  step: f32,
  label: &str,
  handle: Color,
  enabled: bool,
  on_change: Callback<f32>,
) -> View {
  let width = AREA.0;
  let part = (value / max).clamp(0.0, 1.0);
  let set = {
    let on_change = on_change.clone();
    move |v: f32| on_change.call(v.clamp(0.0, max))
  };
  let pointer = {
    let set = set.clone();
    PointerHandler::new(move |e| {
      if matches!(e.kind, PointerKind::Down | PointerKind::Move) {
        set((e.x / width).clamp(0.0, 1.0) * max);
      }
    })
  };
  let keys = {
    let set = set.clone();
    KeyHandler::new(move |e| {
      let big = if e.modifiers.shift { 10.0 } else { 1.0 };
      let v = match &e.key {
        KeyCode::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => {
          value + step * big
        },
        KeyCode::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => {
          value - step * big
        },
        KeyCode::Named(NamedKey::Home) => 0.0,
        KeyCode::Named(NamedKey::End) => max,
        _ => return false,
      };
      set(v);
      true
    })
  };
  let access = Callback::new(move |a: AccessAction| match a {
    AccessAction::Increment => set(value + step),
    AccessAction::Decrement => set(value - step),
    AccessAction::SetNumber(v) => set(v),
    _ => {},
  });
  let e = Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fixed(width),
      height: Size::Fixed(TRACK),
      ..Style::default()
    })
    .role(Role::Slider)
    .label(label.to_owned())
    .value_in(value, ValueRange { min: 0.0, max, step })
    .children([
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(width),
          height: Size::Fixed(TRACK),
          radius: TRACK / 2.0,
          clip: true,
          ..Style::default()
        })
        .children([View::from(
          picture
            .picture()
            .sized(width, TRACK)
            .fit(dovetail::Fit::Stretch),
        )])
        .into(),
      ring(part * width, TRACK / 2.0, handle),
    ]);
  if enabled {
    e.focusable(true)
      .on_pointer(pointer)
      .on_key(keys)
      .on_access(access)
      .into()
  } else {
    e.disabled(true).into()
  }
}

/// A small square showing `colour` over the checks, pressed to choose it.
fn swatch(
  colour: Color,
  size: f32,
  on_press: Option<Callback<()>>,
) -> View {
  let t = theme();
  let (_, light, dark) = t.colour_picker_look();
  let half = size / 2.0;
  let square = |c: Color, x: f32, y: f32| -> View {
    Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(half),
        height: Size::Fixed(half),
        background: c,
        offset: (x, y),
        ..Style::default()
      })
      .into()
  };
  let mut e = Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fixed(size),
      height: Size::Fixed(size),
      radius: 4.0,
      clip: true,
      border: Border {
        width: 1.0,
        color: t.colors.outline_variant,
      },
      ..Style::default()
    })
    .children([
      square(light, 0.0, 0.0),
      square(dark, half, 0.0),
      square(dark, 0.0, half),
      square(light, half, half),
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(size),
          height: Size::Fixed(size),
          background: colour,
          ..Style::default()
        })
        .into(),
    ]);
  if let Some(on_press) = on_press {
    e = e
      .role(Role::Button)
      .label(to_hex(colour))
      .focusable(true)
      .on_press(on_press);
  }
  e.into()
}

/// A colour picker (M25): a colour area of saturation and value, sliders
/// for hue and opacity, and fields for hex, RGB and HSV, all kept in step.
///
/// The arrow keys move the point in the colour area (with Shift, ten times
/// as far). `swatches` are colours the application gives, and `recent` the
/// colours used last, which the application keeps; pressing one chooses
/// it. Without `alpha`, colours are solid. Changing calls `on_change`; the
/// component that owns the colour updates `value`. A typed hex value that
/// is not a colour is refused.
#[component]
pub fn ColourPicker(
  value: Color,
  #[prop(default = Callback::new(|_: Color| {}))] on_change: Callback<Color>,
  #[prop(default)] swatches: Vec<Color>,
  #[prop(default)] recent: Vec<Color>,
  #[prop(default = true)] alpha: bool,
  #[prop(into, default = "Colour")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  // The colour as HSV: kept, so the hue stays while the colour is grey.
  let hsv = use_state(move || Hsv::from_color(value));
  let images =
    use_state(|| std::rc::Rc::new(std::cell::RefCell::new(Images::default())))
      .get();
  let hex_draft = use_state(|| None::<String>);
  let hex_refused = use_state(|| false);
  let mut now = hsv.get();
  if to_hex(now.to_color()) != to_hex(value) {
    // Changed from outside. A grey keeps the hue it had.
    now = keep_hue(Hsv::from_color(value), now.h);
  }
  let set = {
    let on_change = on_change.clone();
    move |new: Hsv| {
      let new = Hsv {
        a: if alpha { new.a } else { 1.0 },
        ..new
      };
      hsv.set(new);
      let colour = new.to_color();
      if to_hex(colour) != to_hex(value) {
        on_change.call(colour);
      }
    }
  };

  // The images, made again only when what they show changes.
  let (area_image, hue_image, alpha_image) = {
    let mut im = images.borrow_mut();
    let hue = now.h.round() as u32 % 360;
    if im.area.as_ref().is_none_or(|(h, _)| *h != hue) {
      let n = AREA_PIXELS;
      let h = now.h;
      im.area = Some((
        hue,
        image(n, n, |x, y| {
          Hsv {
            h,
            s: x as f32 / (n - 1) as f32,
            v: 1.0 - y as f32 / (n - 1) as f32,
            a: 1.0,
          }
          .to_color()
        }),
      ));
    }
    if im.hue.is_none() {
      im.hue = Some(image(360, 1, |x, _| {
        Hsv { h: x as f32, s: 1.0, v: 1.0, a: 1.0 }.to_color()
      }));
    }
    let solid = Hsv { a: 1.0, ..now }.to_color();
    let key = to_hex(solid);
    if im.alpha.as_ref().is_none_or(|(k, _)| *k != key) {
      let (w, h) = (110, 7);
      im.alpha = Some((
        key,
        image(w, h, |x, y| {
          over(
            Color { a: x as f32 / (w - 1) as f32, ..solid },
            checks(x, y),
          )
        }),
      ));
    }
    (
      im.area.as_ref().map(|(_, b)| b.clone()),
      im.hue.clone(),
      im.alpha.as_ref().map(|(_, b)| b.clone()),
    )
  };

  // The colour area: saturation across, value up.
  let area_pointer = {
    let set = set.clone();
    PointerHandler::new(move |e| {
      if matches!(e.kind, PointerKind::Down | PointerKind::Move) {
        let s = (e.x / AREA.0).clamp(0.0, 1.0);
        let v = 1.0 - (e.y / AREA.1).clamp(0.0, 1.0);
        set(Hsv { s, v, ..now });
      }
    })
  };
  let area_keys = {
    let set = set.clone();
    KeyHandler::new(move |e| {
      let by = if e.modifiers.shift { 0.1 } else { 0.01 };
      let (ds, dv) = match &e.key {
        KeyCode::Named(NamedKey::ArrowLeft) => (-by, 0.0),
        KeyCode::Named(NamedKey::ArrowRight) => (by, 0.0),
        KeyCode::Named(NamedKey::ArrowUp) => (0.0, by),
        KeyCode::Named(NamedKey::ArrowDown) => (0.0, -by),
        _ => return false,
      };
      set(Hsv {
        s: (now.s + ds).clamp(0.0, 1.0),
        v: (now.v + dv).clamp(0.0, 1.0),
        ..now
      });
      true
    })
  };
  let solid = Hsv { a: 1.0, ..now }.to_color();
  let mut area = Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fixed(AREA.0),
      height: Size::Fixed(AREA.1),
      ..Style::default()
    })
    .role(Role::Slider)
    .label("Saturation and brightness")
    .value_in(
      (now.s * 100.0).round(),
      ValueRange { min: 0.0, max: 100.0, step: 1.0 },
    )
    .description(format!(
      "Saturation {}%, brightness {}%",
      (now.s * 100.0).round(),
      (now.v * 100.0).round()
    ))
    .children([
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(AREA.0),
          height: Size::Fixed(AREA.1),
          radius: t.shapes.small,
          clip: true,
          ..Style::default()
        })
        .children(area_image.map(|b| {
          View::from(
            b.picture()
              .sized(AREA.0, AREA.1)
              .fit(dovetail::Fit::Stretch),
          )
        }))
        .into(),
      ring(now.s * AREA.0, (1.0 - now.v) * AREA.1, solid),
    ]);
  if enabled {
    area = area
      .focusable(true)
      .on_pointer(area_pointer)
      .on_key(area_keys);
  } else {
    area = area.disabled(true);
  }
  let area: View = area.into();

  let hue_slider = hue_image.map_or(View::Empty, |b| {
    let set = set.clone();
    track(
      &b,
      now.h,
      359.0,
      1.0,
      "Hue",
      Hsv { s: 1.0, v: 1.0, a: 1.0, ..now }.to_color(),
      enabled,
      Callback::new(move |h: f32| set(Hsv { h, ..now })),
    )
  });
  let alpha_slider = match (alpha, alpha_image) {
    (true, Some(b)) => {
      let set = set.clone();
      track(
        &b,
        (now.a * 100.0).round(),
        100.0,
        1.0,
        "Opacity",
        now.to_color(),
        enabled,
        Callback::new(move |a: f32| set(Hsv { a: a / 100.0, ..now })),
      )
    },
    _ => View::Empty,
  };

  // The fields.
  let colour = now.to_color();
  let number =
    |name: &str, value: f32, max: f32, change: Callback<f32>| -> View {
      let name = name.to_owned();
      Element::new(ElementKind::Column)
        .style(Style {
          gap: 2.0,
          font_size: Some(t.text.body_small.size),
          ..Style::default()
        })
        .children([
          View::text(name.clone()),
          ui! {
            NumberField {
              value: value,
              min: 0.0,
              max: max,
              width: 64.0,
              label: name,
              enabled: enabled,
              on_change: move |v: f32| change.call(v),
            }
          },
        ])
        .into()
    };
  let rgb_field = |name: &str, part: usize| -> View {
    let set = set.clone();
    let parts = [colour.r, colour.g, colour.b];
    number(
      name,
      f32::from(to_byte(parts[part])),
      255.0,
      Callback::new(move |v: f32| {
        let mut c = colour;
        let v = (v / 255.0).clamp(0.0, 1.0);
        match part {
          0 => c.r = v,
          1 => c.g = v,
          _ => c.b = v,
        }
        set(keep_hue(Hsv::from_color(c), now.h));
      }),
    )
  };
  let hsv_field = |name: &str, part: usize| -> View {
    let set = set.clone();
    let (value, max) = match part {
      0 => (now.h.round(), 359.0),
      1 => ((now.s * 100.0).round(), 100.0),
      _ => ((now.v * 100.0).round(), 100.0),
    };
    number(
      name,
      value,
      max,
      Callback::new(move |v: f32| {
        set(match part {
          0 => Hsv { h: v, ..now },
          1 => Hsv { s: v / 100.0, ..now },
          _ => Hsv { v: v / 100.0, ..now },
        })
      }),
    )
  };
  let hex_commit = {
    let set = set.clone();
    move || {
      let Some(text) = hex_draft.get() else {
        return;
      };
      match parse_hex(&text) {
        Some(c) => {
          hex_draft.set(None);
          hex_refused.set(false);
          set(keep_hue(Hsv::from_color(c), now.h));
        },
        None => hex_refused.set(true),
      }
    }
  };
  let hex_text = hex_draft.get().unwrap_or_else(|| to_hex(colour));
  let hex_field: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 2.0,
      font_size: Some(t.text.body_small.size),
      ..Style::default()
    })
    .on_key({
      let commit = hex_commit.clone();
      KeyHandler::new(move |e| {
        if e.is(NamedKey::Enter) {
          commit();
          true
        } else {
          false
        }
      })
    })
    .on_focus_within(Callback::new(move |inside: bool| {
      if !inside {
        hex_commit();
      }
    }))
    .children([
      View::text("Hex"),
      ui! {
        TextField {
          value: hex_text,
          width: 100.0,
          label: "Hex",
          on_change: move |text: String| {
            hex_draft.set(Some(text));
            hex_refused.set(false);
          },
        }
      },
    ])
    .into();
  let row = |children: Vec<View>| -> View {
    Element::new(ElementKind::Row)
      .style(Style {
        gap: t.spacing.of(1.5),
        align: Align::End,
        ..Style::default()
      })
      .children(children)
      .into()
  };
  let swatch_row = |name: &str, colours: &[Color]| -> View {
    if colours.is_empty() {
      return View::Empty;
    }
    let set = set.clone();
    Element::new(ElementKind::Row)
      .style(Style {
        gap: t.spacing.of(1.0),
        align: Align::Center,
        ..Style::default()
      })
      .role(Role::Group)
      .label(name.to_owned())
      .children(colours.iter().map(|&c| {
        let set = set.clone();
        swatch(
          c,
          20.0,
          enabled.then(|| {
            Callback::new(move |()| {
              set(keep_hue(Hsv::from_color(c), now.h));
            })
          }),
        )
      }))
      .into()
  };
  let mut parts: Vec<View> = vec![
    area,
    hue_slider,
    alpha_slider,
    row(vec![swatch(colour, 32.0, None), hex_field]),
  ];
  if hex_refused.get() {
    parts.push(
      Element::new(ElementKind::Box)
        .style(Style {
          text_color: Some(t.colors.error),
          font_size: Some(t.text.body_small.size),
          ..Style::default()
        })
        .role(Role::Label)
        .children([View::text("Type a colour as #rrggbb.")])
        .into(),
    );
  }
  parts.push(row(vec![
    rgb_field("R", 0),
    rgb_field("G", 1),
    rgb_field("B", 2),
  ]));
  parts.push(row(vec![
    hsv_field("H", 0),
    hsv_field("S", 1),
    hsv_field("V", 2),
  ]));
  parts.push(swatch_row("Colours", &swatches));
  parts.push(swatch_row("Recent colours", &recent));
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(2.0),
      text_color: Some(t.colors.on_surface),
      ..Style::default()
    })
    .role(Role::Group)
    .label(label)
    .children(parts)
    .into()
}

/// A swatch that shows a colour, and opens a [`ColourPicker`] under it
/// when pressed (M25). Escape or a click outside closes the picker.
/// Assistive tools read it as a colour well with the colour's hex value.
#[component]
pub fn ColourButton(
  value: Color,
  #[prop(default = Callback::new(|_: Color| {}))] on_change: Callback<Color>,
  #[prop(default)] swatches: Vec<Color>,
  #[prop(default)] recent: Vec<Color>,
  #[prop(default = true)] alpha: bool,
  #[prop(into, default = "Colour")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let open = use_state(|| false);
  let mut inside = vec![swatch(value, 22.0, None)];
  if open.get() && enabled {
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Fill),
          ..Style::default()
        })
        .on_press(Callback::new(move |()| open.set(false)))
        .into(),
    );
    let (background, _, radius) = t.dialog_look();
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Below),
          padding: Edges::all(t.spacing.of(3.0)),
          background,
          border: Border {
            width: 1.0,
            color: t.colors.outline_variant,
          },
          radius,
          ..Style::default()
        })
        .on_press(Callback::new(|()| {}))
        .on_key(KeyHandler::new(move |e| {
          if e.is(NamedKey::Escape) {
            open.set(false);
            true
          } else {
            false
          }
        }))
        .children([ui! {
          ColourPicker {
            value: value,
            on_change: {
              let on_change = on_change.clone();
              move |c: Color| on_change.call(c)
            },
            swatches: swatches.clone(),
            recent: recent.clone(),
            alpha: alpha,
            label: label.clone(),
          }
        }])
        .into(),
    );
  }
  let button = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(3.0),
      radius: 6.0,
      border: Border { width: 1.0, color: t.colors.outline },
      ..Style::default()
    })
    .role(Role::ColourWell)
    .label(label)
    .text_value(to_hex(value))
    .expanded(open.get())
    .children(inside);
  if enabled {
    button
      .focusable(true)
      .on_press(Callback::new(move |()| open.set(!open.get())))
      .into()
  } else {
    button.disabled(true).into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn colours_over_checks_mix_by_their_opacity() {
    // Half-clear red over solid blue.
    let a = Hsv { h: 0.0, s: 1.0, v: 1.0, a: 0.5 }.to_color();
    let under = Hsv { h: 240.0, s: 1.0, v: 1.0, a: 1.0 }.to_color();
    let mixed = over(a, under);
    assert!((mixed.r - 0.5).abs() < 1e-6 && (mixed.b - 0.5).abs() < 1e-6);
    let bitmap = image(4, 2, |x, _| {
      Hsv {
        h: 0.0,
        s: 1.0,
        v: x as f32 / 3.0,
        a: 1.0,
      }
      .to_color()
    });
    assert_eq!((bitmap.width(), bitmap.height()), (4, 2));
  }
}
