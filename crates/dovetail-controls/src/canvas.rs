//! Helpers for canvases (M09): a pan and zoom camera, and the rectangle of
//! a dragged selection box. They are plain arithmetic, with no drawing.

/// Where a canvas looks: the world point at its top-left corner, and how
/// many logical pixels one world unit takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
  /// The world point shown at the canvas's top-left corner.
  pub origin: (f32, f32),
  /// Logical pixels per world unit: 2 shows things twice as big.
  pub zoom: f32,
}

impl Default for Camera {
  fn default() -> Self {
    Camera { origin: (0.0, 0.0), zoom: 1.0 }
  }
}

impl Camera {
  /// The smallest and largest zoom that [`Camera::zoom_at`] allows.
  pub const ZOOM_RANGE: (f32, f32) = (0.02, 200.0);

  /// The world point under a point of the canvas, in logical pixels from
  /// its top-left corner.
  pub fn to_world(
    &self,
    (x, y): (f32, f32),
  ) -> (f32, f32) {
    (self.origin.0 + x / self.zoom, self.origin.1 + y / self.zoom)
  }

  /// Where a world point is in the canvas, in logical pixels.
  pub fn to_screen(
    &self,
    (x, y): (f32, f32),
  ) -> (f32, f32) {
    (
      (x - self.origin.0) * self.zoom,
      (y - self.origin.1) * self.zoom,
    )
  }

  /// Zooms by `factor` (2 is twice as big) about a point of the canvas,
  /// which keeps showing the same world point. The zoom stays within
  /// [`Camera::ZOOM_RANGE`].
  pub fn zoom_at(
    &self,
    point: (f32, f32),
    factor: f32,
  ) -> Camera {
    let (min, max) = Self::ZOOM_RANGE;
    let zoom = (self.zoom * factor).clamp(min, max);
    let world = self.to_world(point);
    Camera {
      origin: (world.0 - point.0 / zoom, world.1 - point.1 / zoom),
      zoom,
    }
  }

  /// The zoom factor for a wheel movement of `dy` logical pixels (one
  /// notch is 48): smooth, so many small trackpad movements add up to
  /// the same as one large one. Positive `dy`, towards the user, zooms out.
  pub fn wheel_factor(dy: f32) -> f32 {
    2f32.powf(-dy / 240.0)
  }

  /// Moves the view by (`dx`, `dy`) logical pixels: what was under the
  /// pointer follows a drag.
  pub fn pan(
    &self,
    (dx, dy): (f32, f32),
  ) -> Camera {
    Camera {
      origin: (
        self.origin.0 - dx / self.zoom,
        self.origin.1 - dy / self.zoom,
      ),
      zoom: self.zoom,
    }
  }

  /// A camera that shows the world rectangle `(x0, y0, x1, y1)` whole and
  /// centred in a canvas of `size` logical pixels, with `margin` pixels
  /// round it.
  pub fn fit(
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    size: (f32, f32),
    margin: f32,
  ) -> Camera {
    let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
    let room = (
      (size.0 - 2.0 * margin).max(1.0),
      (size.1 - 2.0 * margin).max(1.0),
    );
    let zoom = (room.0 / w).min(room.1 / h);
    let centre = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    Camera {
      origin: (
        centre.0 - size.0 / 2.0 / zoom,
        centre.1 - size.1 / 2.0 / zoom,
      ),
      zoom,
    }
  }
}

/// A selection box being dragged: where the drag started and where the
/// pointer is now, in the same units (canvas pixels or world units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionBox {
  /// Where the drag started.
  pub start: (f32, f32),
  /// Where the pointer is now.
  pub end: (f32, f32),
}

impl SelectionBox {
  /// The box as `(x0, y0, x1, y1)`, with `x0 <= x1` and `y0 <= y1`,
  /// whichever way it was dragged.
  pub fn rect(&self) -> (f32, f32, f32, f32) {
    (
      self.start.0.min(self.end.0),
      self.start.1.min(self.end.1),
      self.start.0.max(self.end.0),
      self.start.1.max(self.end.1),
    )
  }

  /// True when the pointer has moved less than `slop` from where it
  /// started, so the drag is a click.
  pub fn is_click(
    &self,
    slop: f32,
  ) -> bool {
    (self.end.0 - self.start.0).abs() < slop
      && (self.end.1 - self.start.1).abs() < slop
  }

  /// True when the box overlaps the rectangle `(x0, y0, x1, y1)`.
  pub fn touches(
    &self,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
  ) -> bool {
    let (a0, b0, a1, b1) = self.rect();
    x0 <= a1 && x1 >= a0 && y0 <= b1 && y1 >= b0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn near(
    a: (f32, f32),
    b: (f32, f32),
  ) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
  }

  #[test]
  fn zooming_keeps_the_point_under_the_pointer() {
    let camera = Camera { origin: (100.0, 50.0), zoom: 2.0 };
    let pointer = (300.0, 120.0);
    let under = camera.to_world(pointer);
    for factor in [1.1, 3.0, 0.25, 1000.0, 0.0001] {
      let zoomed = camera.zoom_at(pointer, factor);
      assert!(
        near(zoomed.to_world(pointer), under),
        "at × {factor}: {:?}",
        zoomed.to_world(pointer)
      );
    }
    assert_eq!(camera.zoom_at(pointer, 1000.0).zoom, Camera::ZOOM_RANGE.1);
    // Screen and world convert both ways.
    assert!(near(
      camera.to_screen(camera.to_world((7.0, 9.0))),
      (7.0, 9.0)
    ));
  }

  #[test]
  fn wheel_zoom_adds_up_smoothly() {
    let one = Camera::wheel_factor(48.0);
    let small = Camera::wheel_factor(12.0).powi(4);
    assert!(
      (one - small).abs() < 1e-5,
      "four small moves equal one notch"
    );
    assert!(one < 1.0, "towards the user zooms out");
    assert!((Camera::wheel_factor(-48.0) * one - 1.0).abs() < 1e-6);
  }

  #[test]
  fn panning_moves_what_is_under_the_pointer_with_it() {
    let camera = Camera { origin: (10.0, 10.0), zoom: 4.0 };
    let world = camera.to_world((50.0, 50.0));
    let panned = camera.pan((20.0, -8.0));
    assert!(near(panned.to_screen(world), (70.0, 42.0)));
  }

  #[test]
  fn fit_shows_the_whole_rectangle_centred() {
    let camera = Camera::fit((0.0, 0.0, 200.0, 100.0), (420.0, 420.0), 10.0);
    assert!((camera.zoom - 2.0).abs() < 1e-6);
    assert!(near(camera.to_screen((100.0, 50.0)), (210.0, 210.0)));
    assert!(near(camera.to_screen((0.0, 0.0)), (10.0, 110.0)));
  }

  #[test]
  fn a_selection_box_is_the_same_whichever_way_it_is_dragged() {
    let b = SelectionBox { start: (50.0, 10.0), end: (20.0, 40.0) };
    assert_eq!(b.rect(), (20.0, 10.0, 50.0, 40.0));
    assert!(b.touches((45.0, 35.0, 60.0, 60.0)));
    assert!(!b.touches((51.0, 0.0, 60.0, 60.0)));
    assert!(!b.is_click(3.0));
    assert!(SelectionBox { start: (5.0, 5.0), end: (6.0, 7.0) }.is_click(3.0));
  }
}
