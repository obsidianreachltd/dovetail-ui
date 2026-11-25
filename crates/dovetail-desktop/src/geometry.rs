use dovetail_render::PhysicalSize;

/// A size in logical pixels: physical pixels divided by the display scale.
///
/// Layout uses logical pixels, so a window looks the same size on a normal
/// display and on a high-DPI display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalSize {
  /// Width in logical pixels.
  pub width: f64,
  /// Height in logical pixels.
  pub height: f64,
}

impl LogicalSize {
  /// A logical size.
  pub const fn new(
    width: f64,
    height: f64,
  ) -> Self {
    Self { width, height }
  }

  /// The size in physical pixels at the given display scale, rounded to
  /// whole pixels.
  pub fn to_physical(
    self,
    scale_factor: f64,
  ) -> PhysicalSize {
    let px = |v: f64| (v * scale_factor).round().max(0.0) as u32;
    PhysicalSize {
      width: px(self.width),
      height: px(self.height),
    }
  }

  /// The logical size of a physical size at the given display scale.
  pub fn from_physical(
    size: PhysicalSize,
    scale_factor: f64,
  ) -> Self {
    Self::new(
      f64::from(size.width) / scale_factor,
      f64::from(size.height) / scale_factor,
    )
  }
}

/// The size and display scale of a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowInfo {
  /// The drawing area in physical pixels.
  pub size: PhysicalSize,
  /// The display scale, for example 1.5 for 150%.
  pub scale_factor: f64,
}

impl WindowInfo {
  /// The drawing area in logical pixels.
  pub fn logical_size(&self) -> LogicalSize {
    LogicalSize::from_physical(self.size, self.scale_factor)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn logical_to_physical_at_common_scales() {
    let size = LogicalSize::new(800.0, 600.0);
    assert_eq!(
      size.to_physical(1.0),
      PhysicalSize { width: 800, height: 600 }
    );
    assert_eq!(
      size.to_physical(1.5),
      PhysicalSize { width: 1200, height: 900 }
    );
    assert_eq!(
      size.to_physical(2.0),
      PhysicalSize { width: 1600, height: 1200 }
    );
  }

  #[test]
  fn odd_scales_round_to_whole_pixels() {
    let size = LogicalSize::new(101.0, 33.0);
    assert_eq!(
      size.to_physical(1.25),
      PhysicalSize { width: 126, height: 41 }
    );
  }

  #[test]
  fn physical_to_logical_round_trip() {
    let info = WindowInfo {
      size: PhysicalSize { width: 1200, height: 900 },
      scale_factor: 1.5,
    };
    assert_eq!(info.logical_size(), LogicalSize::new(800.0, 600.0));
  }

  #[test]
  fn minimised_window_has_empty_size() {
    let info = WindowInfo {
      size: PhysicalSize { width: 0, height: 0 },
      scale_factor: 2.0,
    };
    assert!(info.size.is_empty());
    assert_eq!(info.logical_size(), LogicalSize::new(0.0, 0.0));
  }
}
