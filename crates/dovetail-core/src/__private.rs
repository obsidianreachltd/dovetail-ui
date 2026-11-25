//! Support for code that `#[component]` and `ui!` generate. Not for direct
//! use: it may change in any release.
//!
//! A component's generated properties builder tracks which required
//! properties are set in its type parameters, with [`Unset`] and [`Set`].
//! Leaving one out makes `build` unavailable, and the compiler reports the
//! missing property by name.

/// A required property that has not been given yet.
pub struct Unset;

/// A required property that has been given.
pub struct Set;

pub use crate::view::View;

/// Turns anything that can be shown into a view, for `"text"` and `{expr}`
/// nodes in `ui!`.
pub fn into_view(value: impl Into<View>) -> View {
  value.into()
}
