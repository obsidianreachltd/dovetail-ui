//! M17 example: images and icons.
//!
//! - `cargo run -p example-images` opens it. A PNG and a JPEG file show at
//!   their own size, fitted into a square and filling one. "Load a large
//!   image" makes a 4096 × 4096 picture and loads it while the window keeps
//!   drawing. Icons show in the menus, the toolbar, a button, the tabs, a
//!   tree and a dialog, in the light or dark theme. An icon with two
//!   colours keeps its red. An animated GIF and an APNG play, and "Pause"
//!   stops them.
//! - `cargo run -p example-images -- --self-test` checks the pictures'
//!   pixels and the icons' colours in both themes, that every control shows
//!   its icon, that the window keeps drawing while the large image loads,
//!   and that the animations play and pause.

mod self_test;

use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use dovetail::commands::{
  CommandButton, CommandScope, Commands, Menu, MenuBar, Toolbar,
};
use dovetail::controls::{
  Dialog, DialogButton, Icon, Image, ImageSource, ImageState, StatusBar, Tabs,
};
use dovetail::data::{DataTree, RowState, Selection, SelectionMode};
use dovetail::image::{
  ApngFrame, GifFrame, Rgba, encode_apng, encode_gif, encode_png, icons,
};
use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData, theme};
use dovetail::{
  Edges, Element, ElementKind, Fit, Key, Role, Size, Style, TaskEnd, spawn_task,
};

/// A PNG file written by Windows' own encoder, 64 × 48 pixels.
pub(crate) const PNG: &[u8] =
  include_bytes!("../../../crates/dovetail-image/tests/fixtures/gdi.png");
/// The same picture as a JPEG file.
pub(crate) const JPEG: &[u8] =
  include_bytes!("../../../crates/dovetail-image/tests/fixtures/gdi.jpg");
/// The large picture's width and height.
pub(crate) const LARGE: u32 = 4096;

/// What the window shows, for the self-test to read and change.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct State {
  pub(crate) dark: Signal<bool>,
  pub(crate) tab: Signal<usize>,
  pub(crate) dialog: Signal<bool>,
  /// The large picture's file, once made.
  pub(crate) large: Signal<Option<PathBuf>>,
  /// How the large picture is getting on.
  pub(crate) large_state: Signal<String>,
  /// How many of the small pictures are ready.
  pub(crate) ready: Signal<u32>,
  /// The animations play.
  pub(crate) playing: Signal<bool>,
  pub(crate) status: Signal<String>,
}

thread_local! {
  /// The state, for the self-test.
  pub(crate) static STATE: Cell<Option<State>> = const { Cell::new(None) };
}

/// A pattern `LARGE` pixels square as a PNG file in the temporary folder.
/// Made in a task: it is 64 megabytes.
fn make_large() -> std::io::Result<PathBuf> {
  let n = LARGE as usize;
  let mut pixels = vec![0u8; n * n * 4];
  for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
    let (x, y) = (i % n, i / n);
    p.copy_from_slice(&[
      (x / 16) as u8,
      (y / 16) as u8,
      ((x ^ y) / 32) as u8,
      255,
    ]);
  }
  let file = encode_png(&Rgba { width: LARGE, height: LARGE, pixels });
  let path = std::env::temp_dir()
    .join(format!("dovetail-large-{}.png", std::process::id()));
  std::fs::write(&path, file)?;
  Ok(path)
}

/// An icon with two colours of its own (D0024): a page in the text's
/// colour, with a red dot for unsaved changes.
pub(crate) fn unsaved_icon() -> dovetail::image::Icon {
  thread_local! {
    static ICON: dovetail::image::Icon = dovetail::image::Icon::from_svg(
      r##"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M5 3h8l4 4v14H5z"/><circle cx="18" cy="6" r="4.5" fill="#e0262b" stroke="#ffffff" stroke-width="1.5"/></svg>"##,
    )
    .expect("a valid icon");
  }
  ICON.with(Clone::clone)
}

/// The animations' frames: a ball that moves across a band of changing
/// colour, `count` frames 120 ms apart.
fn ball_frames(count: u32) -> Vec<(Rgba, std::time::Duration)> {
  (0..count)
    .map(|i| {
      let (w, h) = (48u32, 48u32);
      let mut pixels = Vec::new();
      let cx = 8.0 + 32.0 * i as f32 / (count - 1) as f32;
      for y in 0..h {
        for x in 0..w {
          let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - 24.0);
          let ball = dx * dx + dy * dy < 36.0;
          pixels.extend(if ball {
            [255, 255, 255, 255]
          } else {
            [
              (40 + 30 * i) as u8,
              90,
              (200 - 25 * i) as u8,
              255,
            ]
          });
        }
      }
      (
        Rgba { width: w, height: h, pixels },
        std::time::Duration::from_millis(120),
      )
    })
    .collect()
}

/// The animation as a GIF file, played for ever.
pub(crate) fn ball_gif() -> Vec<u8> {
  let frames: Vec<GifFrame> = ball_frames(6)
    .into_iter()
    .map(|(image, delay)| GifFrame {
      image,
      left: 0,
      top: 0,
      delay,
      disposal: 1,
      interlaced: false,
    })
    .collect();
  encode_gif(48, 48, &frames, None).expect("few colours")
}

/// The animation as an APNG file, played for ever.
pub(crate) fn ball_apng() -> Vec<u8> {
  let frames: Vec<ApngFrame> = ball_frames(6)
    .into_iter()
    .map(|(image, delay)| ApngFrame {
      image,
      x: 0,
      y: 0,
      delay,
      dispose: 0,
      blend: 0,
    })
    .collect();
  encode_apng(48, 48, &frames, 0)
}

/// A labelled picture in a box `side` pixels square, or at its own size.
fn picture(
  source: ImageSource,
  label: &str,
  fit: Fit,
  side: Option<f32>,
  ready: Signal<u32>,
) -> View {
  let size = side.map_or(Size::Fit, Size::Fixed);
  let t = theme();
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(1.0),
      ..Style::default()
    })
    .children([
      ui! {
        Image {
          source: source,
          label: label,
          fit: fit,
          width: size,
          height: size,
          on_state: move |s: ImageState| {
            if matches!(s, ImageState::Ready(_)) {
              ready.update(|n| *n += 1);
            }
          },
        }
      },
      ui! { Text { value: label, size: Some(t.text.body_small.size) } },
    ])
    .into()
}

#[component]
fn Pictures(state: State) -> View {
  let png = use_state(|| Arc::<[u8]>::from(PNG)).get();
  let jpeg = use_state(|| Arc::<[u8]>::from(JPEG)).get();
  let t = theme();
  let row = |data: &Arc<[u8]>, kind: &str| -> View {
    let bytes = || ImageSource::Bytes(data.clone());
    Element::new(ElementKind::Row)
      .style(Style {
        gap: t.spacing.of(4.0),
        ..Style::default()
      })
      .children([
        picture(
          bytes(),
          &format!("{kind} at its size"),
          Fit::Contain,
          None,
          state.ready,
        ),
        picture(
          bytes(),
          &format!("{kind} fitted"),
          Fit::Contain,
          Some(120.0),
          state.ready,
        ),
        picture(
          bytes(),
          &format!("{kind} filled"),
          Fit::Cover,
          Some(120.0),
          state.ready,
        ),
      ])
      .into()
  };
  let large_state = state.large_state;
  let large = state.large;
  let load = move |_| {
    if large_state.get() != "not loaded" {
      return;
    }
    large_state.set("making the file".to_string());
    spawn_task(
      |_| Some(make_large()),
      None,
      Callback::new(move |end: TaskEnd<std::io::Result<PathBuf>>| match end {
        TaskEnd::Done(Ok(path)) => {
          large_state.set("decoding".to_string());
          large.set(Some(path));
        },
        TaskEnd::Done(Err(e)) => large_state.set(format!("failed: {e}")),
        TaskEnd::Failed(e) => large_state.set(format!("failed: {e}")),
        TaskEnd::Cancelled => {},
      }),
    );
  };
  let large_view = match large.get() {
    Some(path) => ui! {
      Image {
        source: path,
        label: "Large image",
        fit: Fit::Cover,
        width: Size::Fixed(240.0),
        height: Size::Fixed(160.0),
        on_state: move |s: ImageState| large_state.set(match s {
          ImageState::Ready(_) => "ready".to_string(),
          ImageState::Failed(e) => format!("failed: {e}"),
          ImageState::Loading => "decoding".to_string(),
        }),
      }
    },
    None => View::Empty,
  };
  card(vec![
    heading("Images"),
    row(&png, "PNG"),
    row(&jpeg, "JPEG"),
    ui! {
      Row {
        Button { text: "Load a large image", icon: Some(icons::image()), on_click: load }
        Text { value: format!("Large image: {}", large_state.get()) }
      }
    },
    large_view,
  ])
}

fn heading(text: &str) -> View {
  let t = theme();
  Element::new(ElementKind::Box)
    .style(Style {
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      ..Style::default()
    })
    .role(Role::Heading)
    .children([View::text(text)])
    .into()
}

fn card(children: Vec<View>) -> View {
  let t = theme();
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(3.0),
      padding: Edges::all(t.spacing.of(4.0)),
      background: t.colors.surface_container_low,
      radius: t.shapes.large,
      align: dovetail::Align::Start,
      ..Style::default()
    })
    .children(children)
    .into()
}

/// Makes one of Dovetail's icons.
type MakeIcon = fn() -> dovetail::image::Icon;

/// The layers of a drawing, each with an icon.
const LAYERS: [(&str, MakeIcon); 3] = [
  ("Copper", icons::layers),
  ("Silkscreen", icons::eye),
  ("Outline", icons::pointer),
];

#[component]
fn IconsCard(state: State) -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Single)).get();
  let t = theme();
  let gallery: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(2.0),
      ..Style::default()
    })
    .children(icons::ALL.chunks(11).map(|chunk| -> View {
      Element::new(ElementKind::Row)
        .style(Style {
          gap: t.spacing.of(3.0),
          ..Style::default()
        })
        .children(
          chunk
            .iter()
            .map(|(name, icon)| ui! { Icon { icon: icon(), label: *name } }),
        )
        .into()
    }))
    .into();
  let tree = ui! {
    DataTree {
      nodes: |node: Option<&Key>| match node.and_then(Key::number) {
        None => vec![Key::from(100u64)],
        Some(_) => (0..LAYERS.len() as u64).map(Key::from).collect(),
      },
      has_children: |k: &Key| k.number() == Some(100),
      row: |k: &Key, _: &RowState| {
        let (name, icon) = match k.number() {
          Some(i) if i < 100 => (LAYERS[i as usize].0, LAYERS[i as usize].1()),
          _ => ("Board", icons::layers()),
        };
        Element::new(ElementKind::Row)
          .style(Style { gap: 6.0, align: dovetail::Align::Center, ..Style::default() })
          .children([icon.picture().into(), View::text(name)])
          .into()
      },
      text: |k: &Key| match k.number() {
        Some(i) if i < 100 => LAYERS[i as usize].0.to_string(),
        _ => "Board".to_string(),
      },
      selection: selection,
      label: "Layers",
      width: Size::Fixed(260.0),
      height: Size::Fixed(140.0),
    }
  };
  let dialog = state.dialog;
  let tab = state.tab;
  card(vec![
    heading("Icons"),
    ui! {
      Tabs {
        tabs: vec!["Gallery".to_string(), "Layers".to_string()],
        icons: vec![Some(icons::image()), Some(icons::layers())],
        selected: tab.get(),
        on_change: move |i| tab.set(i),
        { if tab.get() == 0 { gallery } else { tree } }
      }
    },
    ui! {
      Row {
        Button { text: "Warn me", icon: Some(icons::warning()), on_click: move |_| dialog.set(true) }
        Icon { icon: crate::unsaved_icon(), size: 32.0, label: "Unsaved changes" }
      }
    },
    heading("Animations"),
    ui! { Animations { playing: state.playing } },
  ])
}

#[component]
fn Animations(playing: Signal<bool>) -> View {
  let gif = use_state(|| Arc::<[u8]>::from(ball_gif())).get();
  let apng = use_state(|| Arc::<[u8]>::from(ball_apng())).get();
  let on = playing.get();
  ui! {
    Row {
      Image { source: ImageSource::Bytes(gif), label: "Animated GIF", playing: on }
      Image { source: ImageSource::Bytes(apng), label: "Animated PNG", playing: on }
      Button {
        text: if on { "Pause" } else { "Play" },
        on_click: move |_| playing.set(!on),
      }
    }
  }
}

#[component]
fn App() -> View {
  let state = State {
    dark: use_state(|| false),
    tab: use_state(|| 0),
    dialog: use_state(|| false),
    large: use_state(|| None),
    large_state: use_state(|| "not loaded".to_string()),
    ready: use_state(|| 0),
    playing: use_state(|| true),
    status: use_state(|| "Ready".to_string()),
  };
  STATE.set(Some(state));
  let commands = use_state(Commands::new).get();
  let status = state.status;
  let said =
    move |what: &'static str| move || status.set(format!("{what} chosen"));
  if commands.info("file.new").is_none() {
    commands
      .define("file.new", "New", Some("Mod+N"), said("New"))
      .define("file.open", "Open", Some("Mod+O"), said("Open"))
      .define("file.save", "Save", Some("Mod+S"), said("Save"))
      .define("edit.undo", "Undo", Some("Mod+Z"), said("Undo"))
      .define("edit.redo", "Redo", Some("Mod+Y"), said("Redo"))
      .define("edit.cut", "Cut", Some("Mod+X"), said("Cut"))
      .define("edit.copy", "Copy", Some("Mod+C"), said("Copy"))
      .define("edit.paste", "Paste", Some("Mod+V"), said("Paste"))
      .with_icon("file.new", icons::new())
      .with_icon("file.open", icons::open())
      .with_icon("file.save", icons::save())
      .with_icon("edit.undo", icons::undo())
      .with_icon("edit.redo", icons::redo())
      .with_icon("edit.cut", icons::cut())
      .with_icon("edit.copy", icons::copy())
      .with_icon("edit.paste", icons::paste());
  }
  let menus = vec![
    Menu::new("File")
      .command("file.new")
      .command("file.open")
      .command("file.save"),
    Menu::new("Edit")
      .command("edit.undo")
      .command("edit.redo")
      .separator()
      .command("edit.cut")
      .command("edit.copy")
      .command("edit.paste"),
  ];
  let ids = [
    "file.new",
    "file.open",
    "file.save",
    "edit.undo",
    "edit.redo",
    "edit.cut",
    "edit.copy",
    "edit.paste",
  ];
  let dark = state.dark;
  let toolbar = ui! {
    Toolbar {
      { View::fragment(ids.iter().map(|id| ui! { CommandButton { commands: commands, id: *id, icon_only: true } })) }
      Button { text: "Light", on_click: move |_| dark.set(false) }
      Button { text: "Dark", on_click: move |_| dark.set(true) }
    }
  };
  let body: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: 16.0,
      padding: Edges::all(16.0),
      grow: 1.0,
      ..Style::default()
    })
    .children([
      ui! { Pictures { state: state } },
      ui! { IconsCard { state: state } },
    ])
    .into();
  let dialog = state.dialog;
  let warning = if dialog.get() {
    ui! {
      Dialog {
        title: "Large files",
        message: "Large images are loaded a part at a time.",
        icon: Some(icons::warning()),
        buttons: vec![DialogButton::new("OK", move || dialog.set(false))],
        on_cancel: move |_| dialog.set(false),
      }
    }
  } else {
    View::Empty
  };
  let window: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { MenuBar { commands: commands, menus: menus } },
      toolbar,
      body,
      ui! { StatusBar { Text { value: status.get() } } },
      warning,
    ])
    .into();
  let data = if dark.get() {
    ThemeData::dark()
  } else {
    ThemeData::light()
  };
  ui! {
    Theme {
      data: data,
      CommandScope { commands: commands, { window } }
    }
  }
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  match dovetail::run("Images", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("images: {e}");
      ExitCode::FAILURE
    },
  }
}
