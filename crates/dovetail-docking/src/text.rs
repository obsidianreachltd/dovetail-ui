//! The saved form of a [`DockLayout`]: plain text of our own (decision
//! D0016).
//!
//! ```text
//! dovetail-dock 1
//! root split row
//!   0.25 tabs 0 files layers
//!   0.75 split column
//!     0.7 tabs 0 board
//!     0.3 tabs 0 console
//! float 120 80 320 400 home=board 0 inspector
//! ```
//!
//! - The first line names the format and its version.
//! - `root` is followed by a node, or by `none` when every panel floats.
//! - A node is `tabs ACTIVE PANEL...` or `split row|column`, with its
//!   parts on the lines below, indented two more spaces, each starting with
//!   its share.
//! - `float X Y WIDTH HEIGHT home=PANEL ACTIVE PANEL...` is a floating
//!   stack. `X Y` is `- -` when the place on the screen is not known, and
//!   `home=-` when it has no panel to go back to.
//!
//! Panel ids cannot contain spaces, and cannot be `-`.

use std::fmt::{self, Write};

use dovetail::WindowPlace;

use crate::model::{Axis, DockLayout, Floating, Node, Tabs};

/// The first line of the format.
const HEADER: &str = "dovetail-dock 1";

/// Why a saved layout could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
  /// The line, counting from 1.
  pub line: usize,
  /// What was wrong.
  pub message: String,
}

impl fmt::Display for ReadError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    write!(f, "line {}: {}", self.line, self.message)
  }
}

impl std::error::Error for ReadError {}

/// True when `id` can be a panel's id in the saved form.
pub fn valid_panel_id(id: &str) -> bool {
  !id.is_empty()
    && id != "-"
    && !id.contains(char::is_whitespace)
    && !id.starts_with("home=")
}

impl DockLayout {
  /// The layout as text, to save.
  pub fn to_text(&self) -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    out.push_str("root ");
    match &self.root {
      None => out.push_str("none\n"),
      Some(node) => write_node(&mut out, node, 0),
    }
    for f in &self.floating {
      let (x, y) = match f.place.and_then(|p| p.position) {
        Some((x, y)) => (number(x), number(y)),
        None => ("-".to_string(), "-".to_string()),
      };
      let (w, h) = f.place.map_or((360.0, 300.0), |p| p.size);
      let _ = writeln!(
        out,
        "float {x} {y} {} {} home={} {} {}",
        number(w),
        number(h),
        f.home.as_deref().unwrap_or("-"),
        f.tabs.active,
        f.tabs.panels.join(" ")
      );
    }
    out
  }

  /// Reads a layout saved with [`DockLayout::to_text`].
  pub fn from_text(text: &str) -> Result<DockLayout, ReadError> {
    let lines: Vec<(usize, usize, &str)> = text
      .lines()
      .enumerate()
      .filter(|(_, l)| !l.trim().is_empty())
      .map(|(n, l)| {
        let indent = l.len() - l.trim_start_matches(' ').len();
        (n + 1, indent, l.trim())
      })
      .collect();
    let error = |line: usize, message: &str| ReadError {
      line,
      message: message.to_string(),
    };
    let Some(&(_, _, first)) = lines.first() else {
      return Err(error(1, "the text is empty"));
    };
    if first != HEADER {
      return Err(error(
        lines[0].0,
        "this is not a docking layout, or a newer version",
      ));
    }
    let mut reader = Reader { lines: &lines, at: 1 };
    let mut layout = DockLayout::default();
    let mut root_seen = false;
    while let Some(&(line, indent, body)) = reader.lines.get(reader.at) {
      if indent != 0 {
        return Err(error(line, "this line is indented too far"));
      }
      reader.at += 1;
      if let Some(rest) = body.strip_prefix("root ") {
        if root_seen {
          return Err(error(line, "there is a second root"));
        }
        root_seen = true;
        if rest != "none" {
          layout.root = Some(reader.node(line, 0, rest)?);
        }
      } else if let Some(rest) = body.strip_prefix("float ") {
        let words: Vec<&str> = rest
          .split(' ')
          .filter(|w| !w.is_empty())
          .collect();
        if words.len() < 7 {
          return Err(error(
            line,
            "a float needs a place, a home and its tabs",
          ));
        }
        let num = |w: &str| {
          w.parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| error(line, &format!("{w:?} is not a number")))
        };
        let position = match (words[0], words[1]) {
          ("-", "-") => None,
          (x, y) => Some((num(x)?, num(y)?)),
        };
        let size = (num(words[2])?.max(80.0), num(words[3])?.max(60.0));
        let home = match words[4].strip_prefix("home=") {
          Some("-") => None,
          Some(h) if valid_panel_id(h) => Some(h.to_string()),
          _ => return Err(error(line, "a float needs home=PANEL or home=-")),
        };
        let tabs = tabs(line, &words[5..])?;
        layout.floating.push(Floating {
          id: layout.floating.len() as u64 + 1,
          tabs,
          place: Some(WindowPlace { position, size }),
          home,
        });
      } else {
        return Err(error(line, "expected root or float"));
      }
    }
    if !root_seen {
      return Err(error(
        lines.last().map_or(1, |l| l.0),
        "there is no root line",
      ));
    }
    let panels = layout.panels();
    for (i, p) in panels.iter().enumerate() {
      if panels[..i].contains(p) {
        return Err(error(1, &format!("panel {p} appears twice")));
      }
    }
    Ok(layout)
  }
}

/// A share or a place, without needless digits.
fn number(v: f32) -> String {
  let s = format!("{:.4}", v);
  let s = s.trim_end_matches('0').trim_end_matches('.');
  if s.is_empty() || s == "-" {
    "0".to_string()
  } else {
    s.to_string()
  }
}

fn write_node(
  out: &mut String,
  node: &Node,
  depth: usize,
) {
  match node {
    Node::Tabs(t) => {
      let _ = writeln!(out, "tabs {} {}", t.active, t.panels.join(" "));
    },
    Node::Split { axis, parts } => {
      let axis = match axis {
        Axis::Row => "row",
        Axis::Column => "column",
      };
      let _ = writeln!(out, "split {axis}");
      for (share, part) in parts {
        let _ = write!(out, "{}{} ", "  ".repeat(depth + 1), number(*share));
        write_node(out, part, depth + 1);
      }
    },
  }
}

fn tabs(
  line: usize,
  words: &[&str],
) -> Result<Tabs, ReadError> {
  let error = |message: &str| ReadError { line, message: message.to_string() };
  let Some((active, panels)) = words.split_first() else {
    return Err(error("tabs need a number and at least one panel"));
  };
  let active: usize = active
    .parse()
    .map_err(|_| error("the shown tab must be a number"))?;
  if panels.is_empty() {
    return Err(error("tabs need at least one panel"));
  }
  if let Some(bad) = panels.iter().find(|p| !valid_panel_id(p)) {
    return Err(error(&format!("{bad:?} cannot be a panel id")));
  }
  Ok(Tabs {
    panels: panels.iter().map(|p| p.to_string()).collect(),
    active: active.min(panels.len() - 1),
  })
}

struct Reader<'a> {
  lines: &'a [(usize, usize, &'a str)],
  at: usize,
}

impl Reader<'_> {
  /// Reads the node described by `body` on `line`, at `depth`, with its
  /// parts on the lines after it.
  fn node(
    &mut self,
    line: usize,
    depth: usize,
    body: &str,
  ) -> Result<Node, ReadError> {
    let error = |line: usize, message: &str| ReadError {
      line,
      message: message.to_string(),
    };
    let words: Vec<&str> = body
      .split(' ')
      .filter(|w| !w.is_empty())
      .collect();
    match words.split_first() {
      Some((&"tabs", rest)) => Ok(Node::Tabs(tabs(line, rest)?)),
      Some((&"split", rest)) => {
        let axis = match rest {
          ["row"] => Axis::Row,
          ["column"] => Axis::Column,
          _ => return Err(error(line, "a split is a row or a column")),
        };
        let mut parts = Vec::new();
        let indent = (depth + 1) * 2;
        while let Some(&(l, i, part)) = self.lines.get(self.at) {
          if i < indent {
            break;
          }
          if i > indent {
            return Err(error(l, "this line is indented too far"));
          }
          self.at += 1;
          let (share, rest) = part
            .split_once(' ')
            .ok_or_else(|| error(l, "a part needs a share and a node"))?;
          let share: f32 = share
            .parse()
            .ok()
            .filter(|s: &f32| s.is_finite() && *s > 0.0)
            .ok_or_else(|| error(l, "a share must be a number above 0"))?;
          parts.push((share, self.node(l, depth + 1, rest)?));
        }
        if parts.len() < 2 {
          return Err(error(line, "a split needs at least two parts"));
        }
        let total: f32 = parts.iter().map(|p| p.0).sum();
        for part in &mut parts {
          part.0 /= total;
        }
        Ok(Node::Split { axis, parts })
      },
      _ => Err(error(line, "expected tabs or split")),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::Place;

  fn sample() -> DockLayout {
    let mut l = DockLayout::new(Node::split(
      Axis::Row,
      vec![
        (0.25, Node::Tabs(Tabs::of(&["files", "layers"]))),
        (
          0.75,
          Node::split(
            Axis::Column,
            vec![
              (0.7, Node::Tabs(Tabs::one("board"))),
              (0.3, Node::Tabs(Tabs::one("console"))),
            ],
          ),
        ),
      ],
    ));
    l.move_panel(
      "layers",
      Place::Float(Some(WindowPlace {
        position: Some((120.0, 80.5)),
        size: (320.0, 400.0),
      })),
    );
    l
  }

  #[test]
  fn the_text_reads_back_the_same() {
    let l = sample();
    let text = l.to_text();
    assert_eq!(
      text,
      "dovetail-dock 1\n\
       root split row\n\
       \x20 0.25 tabs 0 files\n\
       \x20 0.75 split column\n\
       \x20   0.7 tabs 0 board\n\
       \x20   0.3 tabs 0 console\n\
       float 120 80.5 320 400 home=files 0 layers\n"
    );
    assert_eq!(DockLayout::from_text(&text), Ok(l));
  }

  #[test]
  fn every_panel_floating_reads_back() {
    let mut l = DockLayout::new(Node::Tabs(Tabs::one("a")));
    l.move_panel("a", Place::Float(None));
    assert!(l.root.is_none());
    let text = l.to_text();
    assert!(text.contains("root none"));
    let back = DockLayout::from_text(&text).expect("reads");
    assert_eq!(back.panels(), ["a"]);
    assert_eq!(back.floating[0].place.and_then(|p| p.position), None);
  }

  #[test]
  fn bad_text_says_which_line() {
    let cases = [
      ("", 1),
      ("something else\nroot none\n", 1),
      ("dovetail-dock 1\nroot split row\n  0.5 tabs 0 a\n", 2),
      ("dovetail-dock 1\nroot split diagonal\n", 2),
      ("dovetail-dock 1\nroot tabs x a\n", 2),
      ("dovetail-dock 1\nroot tabs 0\n", 2),
      (
        "dovetail-dock 1\nroot split row\n  0.5 tabs 0 a\n    0.5 tabs 0 b\n",
        4,
      ),
      (
        "dovetail-dock 1\nroot split row\n  -1 tabs 0 a\n  1 tabs 0 b\n",
        3,
      ),
      (
        "dovetail-dock 1\nroot tabs 0 a\nfloat 1 2 3 home=- 0 b\n",
        3,
      ),
      ("dovetail-dock 1\nroot tabs 0 a\nwhat\n", 3),
      ("dovetail-dock 1\nfloat - - 300 300 home=- 0 a\n", 2),
    ];
    for (text, line) in cases {
      match DockLayout::from_text(text) {
        Err(e) => assert_eq!(e.line, line, "{text:?}: {e}"),
        Ok(l) => panic!("{text:?} read as {l:?}"),
      }
    }
  }

  #[test]
  fn a_panel_twice_is_refused() {
    let text =
      "dovetail-dock 1\nroot tabs 0 a b\nfloat - - 300 300 home=- 0 a\n";
    assert!(DockLayout::from_text(text).is_err());
  }

  #[test]
  fn shares_are_scaled_and_the_tab_shown_is_kept_in_range() {
    let text =
      "dovetail-dock 1\nroot split column\n  1 tabs 7 a b\n  3 tabs 0 c\n";
    let l = DockLayout::from_text(text).expect("reads");
    let Some(Node::Split { parts, .. }) = &l.root else {
      panic!("a split");
    };
    assert_eq!(parts[0].0, 0.25);
    assert_eq!(
      parts[0].1,
      Node::Tabs(Tabs {
        panels: vec!["a".into(), "b".into()],
        active: 1
      })
    );
  }
}
