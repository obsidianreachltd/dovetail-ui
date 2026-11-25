//! Rendering interfaces and GPU integration for Dovetail UI.
//!
//! This crate owns the GPU. It does not know about winit or any other
//! windowing library: a window is anything wgpu can draw into.
//!
//! - [`Gpu`] holds one GPU device and queue for the whole application. Every
//!   window shares it (decision D0009).
//! - [`WindowSurface`] is the part of one window that the GPU draws into.
//! - Drawing happens only when a frame is asked for (decision D0008).
//! - [`Painter`] paints laid-out boxes into a [`Canvas`] that keeps its
//!   pixels, repainting only damaged regions, and copies it to a window.
//!
//! If the GPU device is lost, [`Gpu::is_lost`] becomes true. Drop every
//! window surface, make a new GPU and new surfaces, and call
//! [`Gpu::replacing`]. Its documentation gives the steps. Painters and
//! canvases belong to one device too: make new ones.

use dovetail_layout::TextStyle;
mod block_on;
mod color;
pub mod font;
mod gpu;
mod paint;
mod surface;

pub use color::Color;
pub use gpu::{Gpu, GpuError};
pub use paint::{
  CANVAS_FORMAT, Canvas, CanvasFrame, CanvasPainter, FOCUS_RING, PaintInput,
  PaintStats, Painter, SELECTION, SELECTION_IDLE, canvas,
};
pub use surface::{FrameOutcome, PhysicalSize, WindowSurface};

/// Measures text the way the [`Painter`] draws it: with the system fonts
/// ([`dovetail_text::Fonts::system`]), or with the built-in stroke font
/// (D0012) when no font can be read.
#[derive(Clone)]
pub struct TextMeasure {
  pub(crate) fonts: dovetail_text::Fonts,
}

impl TextMeasure {
  /// The fonts for text of this weight: the system's fonts for that
  /// weight, or these fonts for regular text (M14).
  fn fonts_for(
    &self,
    font: TextStyle,
  ) -> dovetail_text::Fonts {
    if font.weight == dovetail_layout::FontWeight::REGULAR {
      self.fonts.clone()
    } else {
      dovetail_text::Fonts::system_weight(font.weight.0)
    }
  }
}

impl dovetail_layout::Measure for TextMeasure {
  fn text(
    &self,
    text: &str,
    font: TextStyle,
  ) -> (f32, f32) {
    if self.fonts.is_empty() {
      let stroke = dovetail_layout::FixedPitch {
        advance: font::ADVANCE,
        line: font::LINE,
      };
      return stroke.text(text, font);
    }
    let fonts = self.fonts_for(font);
    // One line per `\n`; the widest line sets the width.
    let line_height = fonts.line_metrics(font.size).line_height();
    let widest = text
      .split('\n')
      .map(|l| fonts.shape(l, font.size).width)
      .fold(0.0, f32::max);
    (widest, line_height * text.split('\n').count() as f32)
  }

  fn caret_x(
    &self,
    line: &str,
    font: TextStyle,
    at: usize,
  ) -> f32 {
    if self.fonts.is_empty() {
      return self.text(&line[..at], font).0;
    }
    self.fonts_for(font).caret_x(line, font.size, at)
  }

  fn byte_at(
    &self,
    line: &str,
    font: TextStyle,
    x: f32,
  ) -> usize {
    if self.fonts.is_empty() {
      let advance = font::ADVANCE * font.size;
      let column = (x / advance).round().max(0.0) as usize;
      return line
        .char_indices()
        .nth(column)
        .map_or(line.len(), |(i, _)| i);
    }
    self.fonts_for(font).byte_at(line, font.size, x)
  }

  fn selection(
    &self,
    line: &str,
    font: TextStyle,
    from: usize,
    to: usize,
  ) -> Vec<(f32, f32)> {
    if self.fonts.is_empty() {
      let a = self.caret_x(line, font, from);
      let b = self.caret_x(line, font, to);
      return vec![(a, b)];
    }
    self
      .fonts_for(font)
      .selection(line, font.size, from, to)
  }

  fn is_rtl(
    &self,
    line: &str,
    font: TextStyle,
  ) -> bool {
    !self.fonts.is_empty() && self.fonts_for(font).shape(line, font.size).rtl
  }

  fn graphemes(
    &self,
    line: &str,
    font: TextStyle,
  ) -> Vec<(usize, f32, f32)> {
    if self.fonts.is_empty() {
      let advance = font::ADVANCE * font.size;
      return line
        .char_indices()
        .enumerate()
        .map(|(n, (i, _))| (i, n as f32 * advance, (n + 1) as f32 * advance))
        .collect();
    }
    self
      .fonts_for(font)
      .shape(line, font.size)
      .clusters
      .iter()
      .map(|c| (c.start, c.left, c.right))
      .collect()
  }
}

/// Measuring that matches what the [`Painter`] draws.
pub fn font_measure() -> TextMeasure {
  TextMeasure { fonts: dovetail_text::Fonts::system() }
}

/// wgpu itself, the exact version Dovetail uses, for canvases that draw
/// with it (M09).
pub use wgpu;
/// Facts about a GPU adapter, such as its name and backend.
pub use wgpu::AdapterInfo;
/// Traits from wgpu that a window must have to be drawn into.
pub use wgpu::DisplayAndWindowHandle;
/// How long [`Gpu::device`]'s `poll` waits, for example in measurements.
pub use wgpu::PollType;
/// The platform display connection that the GPU instance is made with.
pub use wgpu::wgt::WgpuHasDisplayHandle;
