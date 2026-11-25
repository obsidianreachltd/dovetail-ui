//! Reading big-endian numbers from font data, with bounds checks.

/// A view of font data that never panics: every read is checked.
#[derive(Clone, Copy)]
pub(crate) struct Data<'a>(pub &'a [u8]);

impl<'a> Data<'a> {
  pub fn u8(
    self,
    at: usize,
  ) -> Option<u8> {
    self.0.get(at).copied()
  }

  pub fn u16(
    self,
    at: usize,
  ) -> Option<u16> {
    let b = self.0.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
  }

  pub fn i16(
    self,
    at: usize,
  ) -> Option<i16> {
    self.u16(at).map(|v| v as i16)
  }

  pub fn u32(
    self,
    at: usize,
  ) -> Option<u32> {
    let b = self.0.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
  }

  pub fn tag(
    self,
    at: usize,
  ) -> Option<[u8; 4]> {
    let b = self.0.get(at..at.checked_add(4)?)?;
    Some([b[0], b[1], b[2], b[3]])
  }

  /// A 2.14 fixed-point number, as in composite glyph scales.
  pub fn f2dot14(
    self,
    at: usize,
  ) -> Option<f32> {
    self.i16(at).map(|v| f32::from(v) / 16384.0)
  }

  pub fn slice(
    self,
    start: usize,
    len: usize,
  ) -> Option<Data<'a>> {
    self
      .0
      .get(start..start.checked_add(len)?)
      .map(Data)
  }
}

#[cfg(test)]
mod tests {
  use super::Data;

  #[test]
  fn reads_big_endian_and_never_panics() {
    let d = Data(&[0x12, 0x34, 0xff, 0xfe, 0x40, 0x00]);
    assert_eq!(d.u16(0), Some(0x1234));
    assert_eq!(d.i16(2), Some(-2));
    assert_eq!(d.u32(0), Some(0x1234_fffe));
    assert_eq!(d.f2dot14(4), Some(1.0));
    assert_eq!(d.u16(5), None, "past the end");
    assert_eq!(d.u32(usize::MAX - 1), None);
    assert!(d.slice(4, usize::MAX).is_none());
  }
}
