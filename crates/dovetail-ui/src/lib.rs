//! Dovetail UI: a Rust framework for desktop applications on Windows, Linux
//! and macOS.
//!
//! This facade crate re-exports the common parts, so an application needs
//! only one import:
//!
//! ```
//! use dovetail::prelude::*;
//!
//! #[component]
//! fn Greeting(#[prop(into)] name: String) -> View {
//!   ui! { { format!("Hello, {name}") } }
//! }
//!
//! let mut tree = Tree::new();
//! tree.mount(ui! { Greeting { name: "Ada" } });
//! assert_eq!(tree.output(tree.root().unwrap()), Some(&View::text("Hello, Ada")));
//! ```
//!
//! To show it in a window, call [`run`]:
//!
//! ```no_run
//! # use dovetail::prelude::*;
//! # #[component]
//! # fn Greeting(#[prop(into)] name: String) -> View { ui! { { name } } }
//! dovetail::run("Greeting", ui! { Greeting { name: "Ada" } }).expect("window");
//! ```
//!
//! The code that `#[component]` and `ui!` generate refers to this crate as
//! `::dovetail`. The core API is re-exported at the root for that reason.

pub use dovetail_commands as commands;
pub use dovetail_controls as controls;
pub use dovetail_core as core;
pub use dovetail_core::*;
pub use dovetail_data as data;
pub use dovetail_desktop as desktop;
pub use dovetail_docking as docking;
pub use dovetail_documents as documents;
pub use dovetail_image as image;
pub use dovetail_layout as layout;
pub use dovetail_macros::{component, ui};
pub use dovetail_render as render;
pub use dovetail_text as text;
pub use dovetail_theme as theme;

/// The items most applications need, brought in with
/// `use dovetail::prelude::*`. It includes the basic controls.
pub mod prelude {
  pub use dovetail_controls::{
    Button, Column, Row, Stack, Text, TextField, Toggle,
  };
  pub use dovetail_core::{
    BuildContext, Callback, Children, Color, Component, ComponentId, Curve,
    Key, Signal, Transition, Tree, View, use_animated, use_state,
  };
  pub use dovetail_macros::{component, ui};
}

/// Opens one window titled `title` that shows `root`, and runs until it is
/// closed. `root` must be a component view, such as `ui! { App {} }`.
pub fn run(
  title: impl Into<String>,
  root: View,
) -> Result<(), desktop::Error> {
  struct Single {
    options: desktop::WindowOptions,
    root: Option<View>,
  }

  impl desktop::Application for Single {
    fn started(
      &mut self,
      cx: &mut desktop::Context<'_>,
    ) {
      let root = self.root.take().expect("started once");
      if let Err(e) = cx.open_ui_window(self.options.clone(), root) {
        eprintln!("dovetail: {e}");
      }
    }

    fn draw(
      &mut self,
      _: &mut desktop::Context<'_>,
      _: desktop::WindowId,
      _: &mut desktop::Frame,
    ) {
    }
  }

  desktop::run(Single {
    options: desktop::WindowOptions::new(title),
    root: Some(root),
  })
}
