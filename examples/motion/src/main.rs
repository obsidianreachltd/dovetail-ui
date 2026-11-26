//! M18 example: the system's settings, shown as they change, and controls
//! that move unless less motion is set.
//!
//! - `cargo run -p example-motion` opens it. Change less motion, the text
//!   size, the caret blink or the double-click time in the system's
//!   settings, then come back to the window: the values at the top follow,
//!   and so do the controls. The switch slides, the tick and the section
//!   fade, "Move" slides a box, and the picture plays, unless less motion
//!   is set.
//! - `cargo run -p example-motion -- --self-test` sets each setting from
//!   inside the program, so the system's own are not touched, and checks
//!   what changes. It checks that the box ends its move on time, within a
//!   frame, and that no frames are drawn while nothing moves.

mod self_test;

use std::cell::RefCell;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dovetail::controls::{CheckBox, Collapsible, Image, ImageSource};
use dovetail::image::{GifFrame, Rgba, encode_gif};
use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData};
use dovetail::{
  Edges, Element, ElementKind, PointerEvent, PointerHandler, PointerKind, Role,
  Size, Style, now, system_settings,
};

/// How long the box takes to move.
pub(crate) const MOVE_TIME: Duration = Duration::from_millis(300);

/// How far the box moves.
pub(crate) const MOVE_BY: f32 = 200.0;

thread_local! {
  /// When each build of the box happened, and where it was, since its
  /// target last changed; for the self-test.
  pub(crate) static MOVES: RefCell<Vec<(Instant, f32)>> = const { RefCell::new(Vec::new()) };
}

/// The system's settings, as the program sees them now.
#[component]
fn Settings() -> View {
  let s = system_settings();
  let blink = match s.caret_blink {
    Some(t) => format!("{} ms", t.as_millis()),
    None => "off".into(),
  };
  let lines = [
    format!(
      "Less motion: {}",
      if s.reduce_motion { "on" } else { "off" }
    ),
    format!("Text scale: {:.2}", s.text_scale),
    format!("Caret blink: {blink}"),
    format!(
      "Double-click: {} ms, {} px",
      s.double_click_time.as_millis(),
      s.double_click_distance
    ),
    format!(
      "Overlay scroll bars: {}",
      if s.overlay_scrollbars { "yes" } else { "no" }
    ),
  ];
  Element::new(ElementKind::Column)
    .style(Style { gap: 2.0, ..Style::default() })
    .role(Role::Group)
    .label("Settings")
    .children(lines.map(View::text))
    .into()
}

/// A box that slides along when "Move" is pressed, and back again.
#[component]
fn Mover() -> View {
  let away = use_state(|| false);
  let target = if away.get() { MOVE_BY } else { 0.0 };
  let at = use_animated(target, Transition::new(MOVE_TIME, Curve::EaseInOut));
  MOVES.with_borrow_mut(|m| m.push((now(), at)));
  let block: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(24.0),
      height: Size::Fixed(24.0),
      radius: 4.0,
      background: Color::hex(0x1e66f5),
      offset: (at, 0.0),
      ..Style::default()
    })
    .into();
  let lane: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(MOVE_BY + 24.0),
      ..Style::default()
    })
    .children([block])
    .into();
  ui! {
    Row {
      Button {
        text: "Move",
        on_click: move |_| {
          MOVES.with_borrow_mut(Vec::clear);
          away.update(|a| *a = !*a);
        },
      }
      { lane }
      Text { value: format!("Box at {at:.0} px") }
    }
  }
}

/// A box that counts the clicks of each press: 2 for a double-click.
#[component]
fn Pad() -> View {
  let clicks = use_state(|| 0u32);
  let pad: View = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(8.0),
      background: Color::hex(0xdce0e8),
      ..Style::default()
    })
    .role(Role::Button)
    .label("Pad")
    .on_pointer(PointerHandler::new(move |e: &PointerEvent| {
      if e.kind == PointerKind::Down {
        clicks.set(e.clicks);
      }
    }))
    .children([View::text("Double-click here")])
    .into();
  ui! {
    Row {
      { pad }
      Text { value: format!("Pad clicks: {}", clicks.get()) }
    }
  }
}

/// Three frames of one colour each, played for ever.
fn spinner() -> Arc<[u8]> {
  let frame = |c: [u8; 3]| GifFrame {
    image: Rgba {
      width: 8,
      height: 8,
      pixels: [c[0], c[1], c[2], 255].repeat(64),
    },
    left: 0,
    top: 0,
    delay: Duration::from_millis(250),
    disposal: 1,
    interlaced: false,
  };
  let frames = [
    frame([0xd2, 0x0f, 0x39]),
    frame([0x40, 0xa0, 0x2b]),
    frame([0x1e, 0x66, 0xf5]),
  ];
  encode_gif(8, 8, &frames, None)
    .expect("a GIF")
    .into()
}

#[component]
fn App() -> View {
  let on = use_state(|| false);
  let name = use_state(String::new);
  let picture = use_state(spinner).get();
  let content: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 12.0,
      padding: Edges::all(20.0),
      width: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { Settings {} },
      ui! {
        Row {
          Toggle { on: on.get(), on_change: move |v| on.set(v), label: "Lights" }
          CheckBox { checked: on.get(), on_change: move |v| on.set(v), label: "Tick" }
        }
      },
      ui! {
        Collapsible {
          title: "More",
          open: on.get(),
          on_toggle: move |v| on.set(v),
          Text { value: "Inside the section" }
        }
      },
      ui! { Mover {} },
      ui! {
        TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
      },
      ui! { Pad {} },
      ui! {
        Image {
          source: ImageSource::Bytes(picture.clone()),
          label: "Spinner",
          width: Size::Fixed(32.0),
          height: Size::Fixed(32.0),
        }
      },
    ])
    .into();
  ui! {
    Theme {
      data: ThemeData::light(),
      { content }
    }
  }
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  match dovetail::run("Motion", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("motion: {e}");
      ExitCode::FAILURE
    },
  }
}
