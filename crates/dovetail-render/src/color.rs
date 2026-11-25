//! Turning Dovetail colours into wgpu colours.

pub use dovetail_core::Color;

/// `color` as wgpu expects it for a surface with the given format.
///
/// sRGB surfaces convert from sRGB themselves, so they get linear values.
/// Other surfaces get the sRGB values unchanged.
pub(crate) fn to_wgpu(
  color: Color,
  format: wgpu::TextureFormat,
) -> wgpu::Color {
  let part = |c: f32| {
    let c = f64::from(c.clamp(0.0, 1.0));
    if format.is_srgb() {
      srgb_to_linear(c)
    } else {
      c
    }
  };
  wgpu::Color {
    r: part(color.r),
    g: part(color.g),
    b: part(color.b),
    a: f64::from(color.a.clamp(0.0, 1.0)),
  }
}

/// One sRGB part as a linear value, both from 0.0 to 1.0.
pub(crate) fn srgb_to_linear(c: f64) -> f64 {
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hex_splits_into_parts() {
    let c = Color::hex(0xff8000);
    assert_eq!((c.r, c.g, c.b, c.a), (1.0, 128.0 / 255.0, 0.0, 1.0));
  }

  #[test]
  fn srgb_surface_gets_linear_values() {
    let c = to_wgpu(
      Color::rgb(0.5, 0.0, 1.0),
      wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    assert!((c.r - 0.214_041).abs() < 1e-5);
    assert_eq!((c.g, c.b), (0.0, 1.0));
  }

  #[test]
  fn plain_surface_keeps_values_and_clamps() {
    let c = to_wgpu(
      Color { r: 0.5, g: -1.0, b: 2.0, a: 1.0 },
      wgpu::TextureFormat::Bgra8Unorm,
    );
    assert_eq!((c.r, c.g, c.b), (0.5, 0.0, 1.0));
  }
}
