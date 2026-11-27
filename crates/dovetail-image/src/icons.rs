//! Dovetail's own icons (M17): drawn for Dovetail on a 24-pixel grid with
//! 2-pixel lines and round ends, and licensed with it.
//!
//! Each function returns the same icon each time it is called, so caches
//! and comparisons see one icon. [`ALL`] lists them by name.

use std::sync::OnceLock;

use crate::Icon;

/// Wraps an icon's shapes in the set's `svg` element: no fill, lines in the
/// icon's colour, 2 pixels wide, with round ends and corners.
macro_rules! icons {
  ($($(#[$doc:meta])* $name:ident => $shapes:literal,)*) => {
    $(
      $(#[$doc])*
      pub fn $name() -> Icon {
        static ICON: OnceLock<Icon> = OnceLock::new();
        ICON
          .get_or_init(|| {
            Icon::from_svg(concat!(
              r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
              $shapes,
              "</svg>"
            ))
            .expect("Dovetail's icons are valid")
          })
          .clone()
      }
    )*

    /// Every icon in the set, by name.
    pub const ALL: &[(&str, fn() -> Icon)] = &[$((stringify!($name), $name),)*];

    /// The SVG text of the icon called `name`, as a file would hold it.
    pub fn svg(name: &str) -> Option<String> {
      match name {
        $(stringify!($name) => Some(concat!(
          r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
          $shapes,
          "</svg>"
        ).to_string()),)*
        _ => None,
      }
    }
  };
}

icons! {
  /// A new document: a page with a folded corner and a plus.
  new => r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5"/><path d="M12.5 11v6M9.5 14h6"/>"#,
  /// Opening a document: an open folder.
  open => r#"<path d="M3 19V5h6l2 2h7v3"/><path d="M3 19l3-9h15l-3 9z"/>"#,
  /// Saving: a disk.
  save => r#"<path d="M5 3h11l3 3v15H5z"/><path d="M8 3v5h7V3"/><path d="M8 21v-7h8v7"/>"#,
  /// Undo: an arrow back round to the left.
  undo => r#"<path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-4"/>"#,
  /// Redo: an arrow round to the right.
  redo => r#"<path d="M15 14l5-5-5-5"/><path d="M20 9H9a5 5 0 0 0 0 10h4"/>"#,
  /// Cut: scissors.
  cut => r#"<circle cx="6" cy="18" r="3"/><circle cx="18" cy="18" r="3"/><path d="M8.2 15.8L19 4M15.8 15.8L5 4"/>"#,
  /// Copy: two pages, one over the other.
  copy => r#"<rect x="9" y="9" width="12" height="12" rx="1"/><path d="M15 9V4a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h5"/>"#,
  /// Paste: a clipboard.
  paste => r#"<path d="M9 4H6v17h12V4h-3"/><rect x="9" y="2" width="6" height="4" rx="1"/>"#,
  /// Search: a magnifying glass.
  search => r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5L21 21"/>"#,
  /// Close: a cross.
  close => r#"<path d="M6 6l12 12M18 6L6 18"/>"#,
  /// A point up, as on a control that opens upwards.
  chevron_up => r#"<path d="M6 15l6-6 6 6"/>"#,
  /// A point down, as on a drop-down list.
  chevron_down => r#"<path d="M6 9l6 6 6-6"/>"#,
  /// A point to the left.
  chevron_left => r#"<path d="M15 6l-6 6 6 6"/>"#,
  /// A point to the right, as on a closed tree node.
  chevron_right => r#"<path d="M9 6l6 6-6 6"/>"#,
  /// An arrow up.
  arrow_up => r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#,
  /// An arrow down.
  arrow_down => r#"<path d="M12 5v14M6 13l6 6 6-6"/>"#,
  /// An arrow to the left.
  arrow_left => r#"<path d="M19 12H5M11 6l-6 6 6 6"/>"#,
  /// An arrow to the right.
  arrow_right => r#"<path d="M5 12h14M13 6l6 6-6 6"/>"#,
  /// A check mark, as in a check box.
  check => r#"<path d="M5 12.5l4.5 4.5L19 7"/>"#,
  /// A plus.
  plus => r#"<path d="M12 5v14M5 12h14"/>"#,
  /// A minus, as in a check box that is partly on.
  minus => r#"<path d="M5 12h14"/>"#,
  /// A filled dot, as in a chosen radio button.
  dot => r#"<circle cx="12" cy="12" r="5" fill="currentColor" stroke="none"/>"#,
  /// A menu: three lines.
  menu => r#"<path d="M4 6h16M4 12h16M4 18h16"/>"#,
  /// More: three dots in a row.
  more => r#"<path d="M5 12h.01M12 12h.01M19 12h.01" stroke-width="3"/>"#,
  /// Information: an i in a circle.
  info => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5h.01"/>"#,
  /// A warning: an exclamation mark in a triangle.
  warning => r#"<path d="M12 3L2 20h20z"/><path d="M12 9v5M12 17h.01"/>"#,
  /// An error: a cross in a circle.
  error => r#"<circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/>"#,
  /// Layers: two sheets, one above the other.
  layers => r#"<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>"#,
  /// Shown: an eye.
  eye => r#"<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#,
  /// A picture: a frame with a hill and a sun.
  image => r#"<rect x="3" y="4" width="18" height="16" rx="1"/><circle cx="8.5" cy="9.5" r="1.5"/><path d="M21 16l-5-5-9 9"/>"#,
  /// The selection tool: a pointer.
  pointer => r#"<path d="M5 3l14 8-6 1.5L10 19z"/>"#,
  /// Zoom in: a magnifying glass with a plus.
  zoom_in => r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5L21 21M10.5 8v5M8 10.5h5"/>"#,
  /// Zoom out: a magnifying glass with a minus.
  zoom_out => r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5L21 21M8 10.5h5"/>"#,
  /// A date: a page of a calendar (M25).
  calendar => r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>"#,
  /// A time: a clock face (M25).
  clock => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
  /// A colour: an artist's palette (M25).
  palette => r#"<path d="M12 3a9 9 0 1 0 0 18c1.5 0 2-1 2-2s-1-1.5-1-2.5 1-1.5 2-1.5h2a4 4 0 0 0 4-4c0-4.5-4-8-9-8z"/><circle cx="7.5" cy="11.5" r="1"/><circle cx="10" cy="7.5" r="1"/><circle cx="15" cy="7.5" r="1"/>"#,
  /// The rectangle tool (M25).
  rectangle => r#"<rect x="4" y="6" width="16" height="12" rx="1"/>"#,
  /// The ellipse tool (M25).
  ellipse => r#"<ellipse cx="12" cy="12" rx="9" ry="6.5"/>"#,
  /// The line tool (M25).
  line => r#"<path d="M5 19L19 5"/>"#,
  /// The pen tool (M25).
  pen => r#"<path d="M4 20l1-5L16 4l4 4L9 19z"/><path d="M13.5 6.5l4 4"/>"#,
  /// The brush tool (M25).
  brush => r#"<path d="M20 4l-9 9"/><path d="M11 13c-2-1-5 0-5 3 0 2-1 3-3 4 4 1 8 0 8-4z"/>"#,
  /// The eraser tool (M25).
  eraser => r#"<path d="M8 20h12"/><path d="M4 15l9-9 6 6-8 8H8z"/><path d="M9 10l6 6"/>"#,
  /// A grid, as View > Grid (M25).
  grid => r#"<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M4 9.33h16M4 14.67h16M9.33 4v16M14.67 4v16"/>"#,
  /// Snapping to something: a magnet (M25).
  magnet => r#"<path d="M6 4v8a6 6 0 0 0 12 0V4"/><path d="M6 8h3M15 8h3M9 4v8a3 3 0 0 0 6 0V4"/>"#,
  /// The hand tool, which moves the view (M25).
  hand => r#"<path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V12M11 11V4.5a1.5 1.5 0 0 1 3 0V12M14 11.5V6a1.5 1.5 0 0 1 3 0v8a7 7 0 0 1-7 7h-.5a6 6 0 0 1-5-2.7L3 15.5a1.5 1.5 0 0 1 2.4-1.8L8 16"/>"#,
  /// Text lined up on the left (M25).
  align_left => r#"<path d="M4 6h16M4 10h10M4 14h16M4 18h10"/>"#,
  /// Text centred (M25).
  align_centre => r#"<path d="M4 6h16M7 10h10M4 14h16M7 18h10"/>"#,
  /// Text lined up on the right (M25).
  align_right => r#"<path d="M4 6h16M10 10h10M4 14h16M10 18h10"/>"#,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_icon_reads_and_draws_inside_its_box() {
    assert_eq!(ALL.len(), 48);
    for (name, icon) in ALL {
      let c = icon().render(24, 24);
      let ink: u32 = c.alpha.iter().map(|&a| u32::from(a)).sum();
      assert!(ink > 255 * 6, "{name} draws something");
      // Nothing touches the outermost pixels: a pixel of room all round.
      for i in 0..24 {
        for (x, y) in [(i, 0), (i, 23), (0, i), (23, i)] {
          assert!(c.alpha[y * 24 + x] < 128, "{name} at ({x}, {y})");
        }
      }
      assert_eq!(icon(), icon(), "{name} is one icon");
      assert!(svg(name).is_some());
    }
  }
}
