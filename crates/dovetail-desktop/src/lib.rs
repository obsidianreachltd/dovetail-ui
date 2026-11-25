//! Windows, event loop, input and platform integration for Dovetail UI.
//!
//! This crate wraps winit behind Dovetail's own types, so a change in winit
//! changes only this crate. An application implements [`Application`] and
//! calls [`run`].
//!
//! ```no_run
//! use dovetail_desktop::{run, Application, Context, Frame, WindowId, WindowOptions};
//! use dovetail_render::Color;
//!
//! struct Hello;
//!
//! impl Application for Hello {
//!     fn started(&mut self, cx: &mut Context<'_>) {
//!         cx.open_window(WindowOptions::new("Hello")).expect("window");
//!     }
//!
//!     fn draw(&mut self, _cx: &mut Context<'_>, _window: WindowId, frame: &mut Frame) {
//!         frame.clear(Color::hex(0x1e66f5));
//!     }
//! }
//!
//! run(Hello).expect("event loop");
//! ```
//!
//! Windows are drawn only when something asks for it (decision D0008): when
//! a window opens, resizes, changes display scale, or when the application
//! calls [`Context::request_redraw`].

mod access;
mod app;
mod appearance;
mod dev;
mod dialogs;
mod geometry;
mod platform;
mod settings;

pub use app::{
  Application, Context, Error, Frame, WindowId, WindowOptions, run,
};
pub use dovetail_core::{KeyCode, KeyEvent, Modifiers, NamedKey};
pub use dovetail_render::PhysicalSize;
pub use geometry::{LogicalSize, WindowInfo};
pub use settings::settings_dir;
