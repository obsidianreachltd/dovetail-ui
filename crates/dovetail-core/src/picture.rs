//! Pictures (M17, D0023): images and icons as leaves of the view tree, as
//! text is.
//!
//! This crate knows only a picture's size, how it fits its box, its colour
//! and its label. What it draws is a value only the renderer opens, such as
//! a decoded image or an icon's outlines from `dovetail-image`.

use std::any::Any;
use std::rc::Rc;

use crate::Color;

/// How a picture fills the box layout gives it, when that is not its own
/// size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Fit {
  /// The whole picture shows, as large as fits, keeping its shape, centred.
  /// The default.
  #[default]
  Contain,
  /// The picture covers the whole box, keeping its shape; what is outside
  /// is cut off, evenly on each side.
  Cover,
  /// The picture is stretched to the box's shape.
  Stretch,
  /// The picture keeps its own size, centred; what is outside is cut off.
  None,
}

/// What kind of picture it is, and what its size depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PictureKind {
  /// Pixels, with a natural size in logical pixels.
  Image {
    /// Width in logical pixels.
    width: f32,
    /// Height in logical pixels.
    height: f32,
  },
  /// Outlines in one colour. Without a size, an icon is as tall as
  /// [`ICON_SCALE`] times the font size around it.
  Icon {
    /// Width over height.
    aspect: f32,
  },
}

/// An icon's height over the font size around it, when it has no size of
/// its own: 16 pixels beside 14-pixel text.
pub const ICON_SCALE: f32 = 8.0 / 7.0;

/// An image or icon to show. Make one with `dovetail-image`, or with
/// [`Picture::new`] for a renderer of your own. Cheap to clone.
///
/// A picture with a [`label`](Picture::label) is an image for assistive
/// tools; one without is decoration, which they skip.
#[derive(Clone)]
pub struct Picture {
  source: Rc<dyn Any>,
  id: u64,
  /// What kind it is, and its natural size or shape.
  pub kind: PictureKind,
  /// Its size in logical pixels, instead of its natural size.
  pub size: Option<(f32, f32)>,
  /// How it fills a box of another size.
  pub fit: Fit,
  /// An icon's colour. Without one it has the colour of the text around it.
  pub color: Option<Color>,
  /// Its name for assistive tools. Without one it is decoration.
  pub label: Option<Rc<str>>,
}

impl Picture {
  /// A picture that draws `source`, which only the renderer opens. `id`
  /// must be the same only for the same thing drawn, so a new one is
  /// painted again.
  pub fn new<T: 'static>(
    source: T,
    id: u64,
    kind: PictureKind,
  ) -> Picture {
    Picture {
      source: Rc::new(source),
      id,
      kind,
      size: None,
      fit: Fit::default(),
      color: None,
      label: None,
    }
  }

  /// What it draws, if that is a `T`.
  pub fn source<T: 'static>(&self) -> Option<&T> {
    self.source.downcast_ref()
  }

  /// The number that tells what it draws apart from other pictures.
  pub fn id(&self) -> u64 {
    self.id
  }

  /// True for an icon.
  pub fn is_icon(&self) -> bool {
    matches!(self.kind, PictureKind::Icon { .. })
  }

  /// Its size in logical pixels, beside text of `font_size`: its own size,
  /// or its natural one.
  pub fn natural_size(
    &self,
    font_size: f32,
  ) -> (f32, f32) {
    if let Some(size) = self.size {
      return size;
    }
    match self.kind {
      PictureKind::Image { width, height } => (width, height),
      PictureKind::Icon { aspect } => {
        let h = (font_size * ICON_SCALE).round();
        (h * aspect, h)
      },
    }
  }

  /// Shown at `width` × `height` logical pixels.
  pub fn sized(
    mut self,
    width: f32,
    height: f32,
  ) -> Picture {
    self.size = Some((width, height));
    self
  }

  /// A square icon `size` logical pixels wide and tall, or an image that
  /// size across, with its height from its shape.
  pub fn size(
    self,
    size: f32,
  ) -> Picture {
    let aspect = match self.kind {
      PictureKind::Image { width, height } => width / height.max(1e-6),
      PictureKind::Icon { aspect } => aspect,
    };
    if self.is_icon() {
      self.sized(size * aspect, size)
    } else {
      self.sized(size, size / aspect)
    }
  }

  /// Fills a box of another size as `fit` says.
  pub fn fit(
    mut self,
    fit: Fit,
  ) -> Picture {
    self.fit = fit;
    self
  }

  /// Drawn in `color`, for an icon.
  pub fn color(
    mut self,
    color: Color,
  ) -> Picture {
    self.color = Some(color);
    self
  }

  /// Named `label` for assistive tools. An empty label makes it decoration.
  pub fn label(
    mut self,
    label: impl Into<Rc<str>>,
  ) -> Picture {
    let label = label.into();
    self.label = (!label.is_empty()).then_some(label);
    self
  }

  /// Decoration, which assistive tools skip. The default.
  pub fn decorative(mut self) -> Picture {
    self.label = None;
    self
  }
}

impl PartialEq for Picture {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.id == other.id
      && self.kind == other.kind
      && self.size == other.size
      && self.fit == other.fit
      && self.color == other.color
      && self.label == other.label
  }
}

impl std::fmt::Debug for Picture {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    f.debug_struct("Picture")
      .field("id", &self.id)
      .field("kind", &self.kind)
      .field("size", &self.size)
      .field("fit", &self.fit)
      .field("label", &self.label)
      .finish()
  }
}

impl From<Picture> for crate::View {
  fn from(picture: Picture) -> Self {
    crate::View::Picture(picture)
  }
}

/// Where a picture of `natural` size is drawn in `box_` (x, y, width,
/// height) as `fit` says, and which part of it: the rectangle drawn into,
/// and the part of the picture, from 0 to 1 across and down.
pub fn fit_rect(
  fit: Fit,
  natural: (f32, f32),
  box_: [f32; 4],
) -> ([f32; 4], [f32; 4]) {
  let [x, y, w, h] = box_;
  let (nw, nh) = (natural.0.max(1e-6), natural.1.max(1e-6));
  let whole = [0.0, 0.0, 1.0, 1.0];
  match fit {
    Fit::Stretch => (box_, whole),
    Fit::Contain => {
      let s = (w / nw).min(h / nh);
      let (dw, dh) = (nw * s, nh * s);
      (
        [
          x + (w - dw) / 2.0,
          y + (h - dh) / 2.0,
          dw,
          dh,
        ],
        whole,
      )
    },
    Fit::Cover | Fit::None => {
      let s = if fit == Fit::Cover {
        (w / nw).max(h / nh)
      } else {
        1.0
      };
      let (dw, dh) = (nw * s, nh * s);
      // The part of the picture that is inside the box.
      let (cw, ch) = ((w / dw).min(1.0), (h / dh).min(1.0));
      let source = [
        (1.0 - cw) / 2.0,
        (1.0 - ch) / 2.0,
        cw,
        ch,
      ];
      let (sw, sh) = (dw.min(w), dh.min(h));
      (
        [
          x + (w - sw) / 2.0,
          y + (h - sh) / 2.0,
          sw,
          sh,
        ],
        source,
      )
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn image(
    w: f32,
    h: f32,
  ) -> Picture {
    Picture::new((), 1, PictureKind::Image { width: w, height: h })
  }

  #[test]
  fn icons_follow_the_font_size() {
    let icon = Picture::new((), 2, PictureKind::Icon { aspect: 1.0 });
    assert_eq!(icon.natural_size(14.0), (16.0, 16.0));
    assert_eq!(icon.clone().size(24.0).natural_size(14.0), (24.0, 24.0));
    assert_eq!(image(40.0, 20.0).size(10.0).natural_size(14.0), (10.0, 5.0));
  }

  #[test]
  fn fitting_a_wide_picture_in_a_square() {
    let b = [0.0, 0.0, 100.0, 100.0];
    let (rect, part) = fit_rect(Fit::Contain, (200.0, 100.0), b);
    assert_eq!(rect, [0.0, 25.0, 100.0, 50.0]);
    assert_eq!(part, [0.0, 0.0, 1.0, 1.0]);
    let (rect, part) = fit_rect(Fit::Cover, (200.0, 100.0), b);
    assert_eq!(rect, b);
    assert_eq!(part, [0.25, 0.0, 0.5, 1.0]);
    let (rect, part) = fit_rect(Fit::None, (50.0, 200.0), b);
    assert_eq!(rect, [25.0, 0.0, 50.0, 100.0]);
    assert_eq!(part, [0.0, 0.25, 1.0, 0.5]);
    assert_eq!(fit_rect(Fit::Stretch, (1.0, 5.0), b).0, b);
  }

  #[test]
  fn an_empty_label_is_decoration() {
    assert_eq!(image(1.0, 1.0).label("").label, None);
    assert_eq!(image(1.0, 1.0).label("Logo").label.as_deref(), Some("Logo"));
  }
}
