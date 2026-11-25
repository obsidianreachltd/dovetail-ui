//! Row heights (M16): where each row starts, and which rows are in sight.
//!
//! Rows of one height are placed by arithmetic. Rows of different heights
//! are measured as they appear; rows not yet seen count as an estimate.
//! Their sums are kept in a Fenwick tree, so finding a row by its offset
//! takes a few steps even with 100,000 rows.

use dovetail::Key;

/// How tall the rows of a view are.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RowHeight {
  /// The theme's row height. The default.
  #[default]
  Theme,
  /// Every row is this many logical pixels tall.
  Fixed(f32),
  /// Rows are as tall as their content, measured as they appear. Rows not
  /// yet seen are taken to be `estimate` tall.
  Measured {
    /// The height of a row not yet measured.
    estimate: f32,
  },
}

/// The heights of a view's rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heights {
  count: usize,
  /// Every row's height, when they are all the same.
  fixed: Option<f32>,
  estimate: f32,
  /// Each row's height so far, and the key it was measured for.
  each: Vec<(f32, Option<Key>)>,
  /// A Fenwick tree over `each`'s heights.
  sums: Vec<f64>,
}

impl Heights {
  /// `count` rows, all `fixed` tall, or `estimate` tall until measured.
  pub fn new(
    count: usize,
    fixed: Option<f32>,
    estimate: f32,
  ) -> Heights {
    let mut h = Heights {
      count: 0,
      fixed,
      estimate: estimate.max(1.0),
      each: Vec::new(),
      sums: Vec::new(),
    };
    h.resize(count);
    h
  }

  /// How many rows there are.
  pub fn len(&self) -> usize {
    self.count
  }

  /// True when there are no rows.
  pub fn is_empty(&self) -> bool {
    self.count == 0
  }

  /// The height of a row not yet measured.
  pub fn estimate(&self) -> f32 {
    self.estimate
  }

  /// True when rows are measured, not all one height.
  pub fn measured(&self) -> bool {
    self.fixed.is_none()
  }

  /// Changes the number of rows. Measured rows that are still there keep
  /// their heights.
  pub fn resize(
    &mut self,
    count: usize,
  ) {
    self.count = count;
    if self.fixed.is_some() {
      return;
    }
    self.each.resize(count, (self.estimate, None));
    self.rebuild();
  }

  fn rebuild(&mut self) {
    let n = self.each.len();
    self.sums = vec![0.0; n + 1];
    for i in 0..n {
      let at = i + 1;
      self.sums[at] += f64::from(self.each[i].0);
      let parent = at + (at & at.wrapping_neg());
      if parent <= n {
        let carried = self.sums[at];
        self.sums[parent] += carried;
      }
    }
  }

  /// Row `index`'s height.
  pub fn height(
    &self,
    index: usize,
  ) -> f32 {
    match self.fixed {
      Some(h) => h,
      None => self
        .each
        .get(index)
        .map_or(self.estimate, |e| e.0),
    }
  }

  /// True when row `index` was measured for the row with `key`. A row
  /// measured for another key, before the data changed, must be measured
  /// again.
  pub fn is_measured(
    &self,
    index: usize,
    key: &Key,
  ) -> bool {
    self.fixed.is_some()
      || self
        .each
        .get(index)
        .is_some_and(|e| e.1.as_ref() == Some(key))
  }

  /// Records that row `index`, for the row `key`, is `height` tall.
  /// Returns true when that changed anything.
  pub fn set(
    &mut self,
    index: usize,
    key: Key,
    height: f32,
  ) -> bool {
    if self.fixed.is_some() || index >= self.count {
      return false;
    }
    let old = self.each[index].0;
    let same_key = self.each[index].1.as_ref() == Some(&key);
    if same_key && (old - height).abs() < 0.5 {
      return false;
    }
    self.each[index] = (height, Some(key));
    let delta = f64::from(height - old);
    let mut at = index + 1;
    while at < self.sums.len() {
      self.sums[at] += delta;
      at += at & at.wrapping_neg();
    }
    true
  }

  /// Where row `index` starts: the height of every row before it.
  pub fn top(
    &self,
    index: usize,
  ) -> f32 {
    let index = index.min(self.count);
    match self.fixed {
      Some(h) => index as f32 * h,
      None => {
        let mut sum = 0.0;
        let mut at = index;
        while at > 0 {
          sum += self.sums[at];
          at -= at & at.wrapping_neg();
        }
        sum as f32
      },
    }
  }

  /// The height of every row.
  pub fn total(&self) -> f32 {
    self.top(self.count)
  }

  /// The row at `offset` down from the first row's top, the last row
  /// below the end, and 0 above the start.
  pub fn index_at(
    &self,
    offset: f32,
  ) -> usize {
    if self.count == 0 || offset <= 0.0 {
      return 0;
    }
    let last = self.count - 1;
    match self.fixed {
      Some(h) => ((offset / h) as usize).min(last),
      None => {
        // Walk down the tree for the last prefix that ends at or before
        // the offset.
        let n = self.sums.len() - 1;
        let mut step = n.next_power_of_two();
        let mut at = 0;
        let mut left = f64::from(offset);
        while step > 0 {
          let next = at + step;
          if next <= n && self.sums[next] <= left {
            at = next;
            left -= self.sums[next];
          }
          step /= 2;
        }
        at.min(last)
      },
    }
  }

  /// The rows to make for a view scrolled `offset` down that shows
  /// `height` pixels: those in sight, and `extra` pixels more on each
  /// side. As a range of indexes.
  pub fn in_sight(
    &self,
    offset: f32,
    height: f32,
    extra: f32,
  ) -> std::ops::Range<usize> {
    if self.count == 0 {
      return 0..0;
    }
    let first = self.index_at(offset - extra);
    let last = self.index_at(offset + height + extra);
    first..(last + 1).min(self.count)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fixed_rows_are_placed_by_arithmetic() {
    let h = Heights::new(100_000, Some(20.0), 0.0);
    assert_eq!(h.top(10), 200.0);
    assert_eq!(h.total(), 2_000_000.0);
    assert_eq!(h.index_at(205.0), 10);
    assert_eq!(h.index_at(5e9), 99_999);
    assert_eq!(h.in_sight(1000.0, 100.0, 20.0), 49..57);
  }

  #[test]
  fn measured_rows_move_the_rows_after_them() {
    let mut h = Heights::new(1000, None, 10.0);
    assert_eq!(h.total(), 10_000.0);
    assert!(!h.is_measured(3, &Key::from(3u64)));
    assert!(h.set(3, Key::from(3u64), 40.0));
    assert!(!h.set(3, Key::from(3u64), 40.2), "a tiny change is ignored");
    assert!(h.is_measured(3, &Key::from(3u64)));
    assert!(
      !h.is_measured(3, &Key::from(4u64)),
      "measured for another row"
    );
    assert_eq!(h.top(3), 30.0);
    assert_eq!(h.top(4), 70.0);
    assert_eq!(h.total(), 10_030.0);
    assert_eq!(h.index_at(35.0), 3);
    assert_eq!(h.index_at(69.0), 3);
    assert_eq!(h.index_at(70.0), 4);
    // Every index agrees with the sums.
    for i in 0..1000 {
      assert_eq!(h.index_at(h.top(i) + 0.5), i, "row {i}");
    }
  }

  #[test]
  fn resizing_keeps_measured_rows() {
    let mut h = Heights::new(10, None, 10.0);
    h.set(2, Key::from(2u64), 30.0);
    h.resize(20);
    assert_eq!(h.height(2), 30.0);
    assert_eq!(h.total(), 220.0);
    h.resize(2);
    assert_eq!(h.total(), 20.0);
  }
}
