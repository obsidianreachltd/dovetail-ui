//! Animated values (M18, decision D0028). [`use_animated`] gives the value
//! to show now, easing from the value shown to a new target over a
//! [`Transition`]'s time. It works out where it is from [`now`], and asks
//! for frames with [`request_frame`] until it gets there. With less motion
//! set it jumps.
//!
//! [`now`]: crate::now
//! [`request_frame`]: crate::request_frame

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::element::{Color, Edges};
use crate::{now, reduce_motion, request_frame, use_state};

/// How a value moves from its start to its end over time: the cubic
/// curves CSS uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Curve {
  /// The same speed all the way.
  Linear,
  /// Starts slowly, as CSS `ease-in`.
  EaseIn,
  /// Ends slowly, as CSS `ease-out`. The usual curve for something that
  /// appears or answers a press.
  #[default]
  EaseOut,
  /// Starts and ends slowly, as CSS `ease-in-out`.
  EaseInOut,
}

impl Curve {
  /// How far along the value is, from 0 to 1, when `t` of the time has
  /// passed, from 0 to 1.
  pub fn at(
    self,
    t: f32,
  ) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match self {
      Curve::Linear => t,
      Curve::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
      Curve::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
      Curve::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
    }
  }
}

/// A CSS `cubic-bezier(x1, y1, x2, y2)` at time `t`: finds where the curve
/// is `t` across, then gives its height there.
fn cubic_bezier(
  x1: f32,
  y1: f32,
  x2: f32,
  y2: f32,
  t: f32,
) -> f32 {
  // Each coordinate is 3(1-s)²s·p1 + 3(1-s)s²·p2 + s³.
  let point = |s: f32, p1: f32, p2: f32| {
    let r = 1.0 - s;
    3.0 * r * r * s * p1 + 3.0 * r * s * s * p2 + s * s * s
  };
  let slope = |s: f32, p1: f32, p2: f32| {
    let r = 1.0 - s;
    3.0 * r * r * p1 + 6.0 * r * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
  };
  // Newton's method from s = t, then halving if it strays.
  let mut s = t;
  for _ in 0..8 {
    let error = point(s, x1, x2) - t;
    if error.abs() < 1e-6 {
      return point(s, y1, y2);
    }
    let d = slope(s, x1, x2);
    if d.abs() < 1e-6 {
      break;
    }
    s -= error / d;
  }
  let (mut low, mut high) = (0.0f32, 1.0f32);
  s = t;
  for _ in 0..32 {
    let x = point(s, x1, x2);
    if (x - t).abs() < 1e-6 {
      break;
    }
    if x < t {
      low = s;
    } else {
      high = s;
    }
    s = (low + high) / 2.0;
  }
  point(s, y1, y2)
}

/// How long a value takes to reach a new target, and its curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Transition {
  /// How long it takes. Zero jumps.
  pub duration: Duration,
  /// How it moves on the way.
  pub curve: Curve,
}

impl Default for Transition {
  fn default() -> Self {
    Transition::NONE
  }
}

impl Transition {
  /// No transition: values jump to their target.
  pub const NONE: Transition = Transition {
    duration: Duration::ZERO,
    curve: Curve::Linear,
  };

  /// A transition of this long, with this curve.
  pub const fn new(
    duration: Duration,
    curve: Curve,
  ) -> Transition {
    Transition { duration, curve }
  }

  /// A transition of `millis` milliseconds that ends slowly.
  pub const fn millis(millis: u64) -> Transition {
    Transition::new(Duration::from_millis(millis), Curve::EaseOut)
  }

  /// How far along a transition is, from 0 to 1, `elapsed` after it
  /// started.
  pub fn progress(
    &self,
    elapsed: Duration,
  ) -> f32 {
    if self.duration.is_zero() || elapsed >= self.duration {
      return 1.0;
    }
    self
      .curve
      .at(elapsed.as_secs_f32() / self.duration.as_secs_f32())
  }
}

/// A value that can move smoothly from one value to another. Dovetail
/// gives it for `f32`, [`Color`], [`Edges`], and pairs and arrays of these.
/// Applications add it for their own types.
pub trait Animate: Clone + PartialEq + 'static {
  /// The value `t` of the way from `self` to `to`. `t` is 0 at `self` and
  /// 1 at `to`, and can go a little outside that with curves that
  /// overshoot.
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self;
}

impl Animate for f32 {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    self + (to - self) * t
  }
}

impl Animate for f64 {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    self + (to - self) * f64::from(t)
  }
}

fn to_linear(c: f32) -> f32 {
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}

fn to_srgb(c: f32) -> f32 {
  if c <= 0.003_130_8 {
    c * 12.92
  } else {
    1.055 * c.powf(1.0 / 2.4) - 0.055
  }
}

/// Colours move in linear light, with their alpha multiplied in, so the
/// half-way colour is not darker than either end, and a colour fading in
/// from clear does not pass through black.
impl Animate for Color {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    let linear = |c: &Color| {
      [
        to_linear(c.r) * c.a,
        to_linear(c.g) * c.a,
        to_linear(c.b) * c.a,
        c.a,
      ]
    };
    let mixed = linear(self).animate(&linear(to), t);
    let a = mixed[3].clamp(0.0, 1.0);
    if a <= 0.0 {
      return Color::TRANSPARENT;
    }
    let part = |p: f32| to_srgb((p / a).clamp(0.0, 1.0));
    Color {
      r: part(mixed[0]),
      g: part(mixed[1]),
      b: part(mixed[2]),
      a,
    }
  }
}

impl Animate for Edges {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    Edges {
      top: self.top.animate(&to.top, t),
      right: self.right.animate(&to.right, t),
      bottom: self.bottom.animate(&to.bottom, t),
      left: self.left.animate(&to.left, t),
    }
  }
}

impl<A: Animate, B: Animate> Animate for (A, B) {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    (self.0.animate(&to.0, t), self.1.animate(&to.1, t))
  }
}

impl<A: Animate, B: Animate, C: Animate> Animate for (A, B, C) {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    (
      self.0.animate(&to.0, t),
      self.1.animate(&to.1, t),
      self.2.animate(&to.2, t),
    )
  }
}

impl<T: Animate, const N: usize> Animate for [T; N] {
  fn animate(
    &self,
    to: &Self,
    t: f32,
  ) -> Self {
    std::array::from_fn(|i| self[i].animate(&to[i], t))
  }
}

/// Where one animated value is going.
struct Moving<T> {
  from: T,
  to: T,
  start: Instant,
  transition: Transition,
}

impl<T: Animate> Moving<T> {
  /// The value at `at`, and whether it has arrived.
  fn at(
    &self,
    at: Instant,
  ) -> (T, bool) {
    let elapsed = at.saturating_duration_since(self.start);
    if elapsed >= self.transition.duration {
      return (self.to.clone(), true);
    }
    let t = self.transition.progress(elapsed);
    (self.from.animate(&self.to, t), false)
  }
}

/// A value that eases to `target` whenever it changes, over `transition`'s
/// time. Call it while a component builds, the same way every build, as
/// [`use_state`]. It returns the value to show now.
///
/// - The first build shows `target` at once.
/// - When `target` changes, the value moves from the value shown to the
///   new target, with the full time again, even part of the way through.
/// - While it moves it asks for frames, so the component is built again
///   at each until it arrives. When it arrives it stops asking.
/// - With less motion set ([`reduce_motion`]), or with
///   [`Transition::NONE`], it jumps to its target. It reads less motion,
///   so the component is built again when that changes.
///
/// Animations that carry meaning, such as a progress bar's value, should
/// still show their values with less motion set; they need no transition.
pub fn use_animated<T: Animate>(
  target: T,
  transition: Transition,
) -> T {
  // State in a cell the component never reads as a signal, so changing
  // it marks nothing: frames build the component again.
  let state = use_state(|| {
    Rc::new(RefCell::new(Moving {
      from: target.clone(),
      to: target.clone(),
      start: now(),
      transition: Transition::NONE,
    }))
  })
  .get();
  let jump = reduce_motion() || transition.duration.is_zero();
  let mut moving = state.borrow_mut();
  let at = now();
  if moving.to != target {
    let (shown, _) = moving.at(at);
    *moving = Moving {
      from: shown,
      to: target,
      start: at,
      transition,
    };
  }
  if jump {
    moving.from = moving.to.clone();
    moving.transition = Transition::NONE;
    return moving.to.clone();
  }
  let (value, arrived) = moving.at(at);
  if !arrived {
    request_frame();
  }
  value
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn curves_start_at_0_and_end_at_1() {
    for curve in [
      Curve::Linear,
      Curve::EaseIn,
      Curve::EaseOut,
      Curve::EaseInOut,
    ] {
      assert_eq!(curve.at(0.0), 0.0, "{curve:?}");
      assert!((curve.at(1.0) - 1.0).abs() < 1e-5, "{curve:?}");
      // Each only moves forward.
      let mut last = 0.0;
      for i in 1..=100 {
        let v = curve.at(i as f32 / 100.0);
        assert!(v >= last - 1e-6, "{curve:?} at {i}");
        last = v;
      }
    }
  }

  #[test]
  fn curves_match_css() {
    // Values from the CSS definitions, worked out separately.
    let near = |a: f32, b: f32| (a - b).abs() < 0.002;
    assert!(
      near(Curve::EaseIn.at(0.5), 0.3153),
      "{}",
      Curve::EaseIn.at(0.5)
    );
    assert!(
      near(Curve::EaseOut.at(0.5), 0.6847),
      "{}",
      Curve::EaseOut.at(0.5)
    );
    assert!(near(Curve::EaseInOut.at(0.5), 0.5));
    assert!(
      near(Curve::EaseInOut.at(0.25), 0.1291),
      "{}",
      Curve::EaseInOut.at(0.25)
    );
    assert!(near(Curve::Linear.at(0.3), 0.3));
  }

  #[test]
  fn colours_mix_in_linear_light() {
    let black = Color::BLACK;
    let white = Color::WHITE;
    let mid = black.animate(&white, 0.5);
    // Half the light is about 0.735 in sRGB, not 0.5.
    assert!((mid.r - 0.735).abs() < 0.01, "{mid:?}");
    // Fading in from clear keeps the colour.
    let red = Color::rgb(1.0, 0.0, 0.0);
    let half = Color::TRANSPARENT.animate(&red, 0.5);
    assert!((half.r - 1.0).abs() < 1e-4 && (half.a - 0.5).abs() < 1e-6);
    assert_eq!(
      Color::TRANSPARENT.animate(&Color::TRANSPARENT, 0.5),
      Color::TRANSPARENT
    );
  }

  #[test]
  fn pairs_arrays_and_edges_move_each_part() {
    assert_eq!((0.0f32, 10.0f32).animate(&(10.0, 20.0), 0.5), (5.0, 15.0));
    assert_eq!([0.0f32, 4.0].animate(&[2.0, 0.0], 0.5), [1.0, 2.0]);
    assert_eq!(
      Edges::all(0.0).animate(&Edges::xy(4.0, 2.0), 0.5),
      Edges::xy(2.0, 1.0)
    );
  }

  #[test]
  fn progress_ends_at_the_duration() {
    let t = Transition::new(Duration::from_millis(100), Curve::Linear);
    assert_eq!(t.progress(Duration::ZERO), 0.0);
    assert_eq!(t.progress(Duration::from_millis(50)), 0.5);
    assert_eq!(t.progress(Duration::from_millis(100)), 1.0);
    assert_eq!(Transition::NONE.progress(Duration::ZERO), 1.0);
  }
}
