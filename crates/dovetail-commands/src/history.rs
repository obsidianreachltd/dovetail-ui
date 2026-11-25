//! Undo and redo: edits made through commands are kept so they can be
//! taken back and done again.

use std::rc::Rc;

use dovetail::{Signal, create_signal};

use crate::commands::Commands;

struct Step {
  name: String,
  undo: Rc<dyn Fn()>,
  redo: Rc<dyn Fn()>,
}

/// The steps behind the signal. (A wrapper, so the handle can derive its
/// traits.)
pub struct Steps {
  done: Vec<Step>,
  undone: Vec<Step>,
  limit: usize,
}

impl std::fmt::Debug for Steps {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(
      f,
      "History({} done, {} undone)",
      self.done.len(),
      self.undone.len()
    )
  }
}

/// An undo history: each edit is done through it, and kept so it can be
/// undone and redone. It is a small handle that can be copied into
/// closures.
///
/// Components that show it (such as the Undo menu item) are built again
/// when it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct History(Signal<Steps>);

impl Default for History {
  fn default() -> Self {
    Self::new()
  }
}

impl History {
  /// An empty history that keeps the last 1,000 edits.
  pub fn new() -> History {
    History(create_signal(Steps {
      done: Vec::new(),
      undone: Vec::new(),
      limit: 1000,
    }))
  }

  /// Does an edit now, and keeps it: `redo` makes the change, and `undo`
  /// takes it back. `name` says what it was, as in "Undo Add item". Doing
  /// an edit forgets what was undone before.
  pub fn apply(
    self,
    name: impl Into<String>,
    redo: impl Fn() + 'static,
    undo: impl Fn() + 'static,
  ) {
    redo();
    let step = Step {
      name: name.into(),
      undo: Rc::new(undo),
      redo: Rc::new(redo),
    };
    self.0.update(|s| {
      s.undone.clear();
      s.done.push(step);
      if s.done.len() > s.limit {
        s.done.remove(0);
      }
    });
  }

  /// Takes back the last edit. Returns whether there was one.
  pub fn undo(self) -> bool {
    let mut step = None;
    self.0.update(|s| step = s.done.pop());
    let Some(step) = step else {
      return false;
    };
    // The edit runs outside the history, as it changes other state.
    (step.undo)();
    self.0.update(|s| s.undone.push(step));
    true
  }

  /// Does the last undone edit again. Returns whether there was one.
  pub fn redo(self) -> bool {
    let mut step = None;
    self.0.update(|s| step = s.undone.pop());
    let Some(step) = step else {
      return false;
    };
    (step.redo)();
    self.0.update(|s| s.done.push(step));
    true
  }

  /// True when there is an edit to undo.
  pub fn can_undo(self) -> bool {
    self.0.map(|s| !s.done.is_empty())
  }

  /// True when there is an edit to redo.
  pub fn can_redo(self) -> bool {
    self.0.map(|s| !s.undone.is_empty())
  }

  /// The name of the edit Undo would take back.
  pub fn undo_name(self) -> Option<String> {
    self
      .0
      .map(|s| s.done.last().map(|e| e.name.clone()))
  }

  /// The name of the edit Redo would do again.
  pub fn redo_name(self) -> Option<String> {
    self
      .0
      .map(|s| s.undone.last().map(|e| e.name.clone()))
  }

  /// Forgets every edit, as after saving a new document.
  pub fn clear(self) {
    self.0.update(|s| {
      s.done.clear();
      s.undone.clear();
    });
  }

  /// Defines Undo (`edit.undo`, Mod+Z) and Redo (`edit.redo`: Mod+Shift+Z
  /// on macOS, Mod+Y elsewhere) for this history, each enabled only when
  /// there is something to undo or redo.
  pub fn add_commands(
    self,
    commands: Commands,
  ) -> Commands {
    let redo_key = if cfg!(target_os = "macos") {
      "Mod+Shift+Z"
    } else {
      "Mod+Y"
    };
    commands
      .define("edit.undo", "Undo", Some("Mod+Z"), move || {
        self.undo();
      })
      .enabled_when("edit.undo", move || self.can_undo())
      .define("edit.redo", "Redo", Some(redo_key), move || {
        self.redo();
      })
      .enabled_when("edit.redo", move || self.can_redo())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn edits_through_commands_undo_and_redo() {
    let (_tree, (list, history, commands)) = crate::owned(|| {
      let list = create_signal(Vec::<String>::new());
      let history = History::new();
      let commands = history.add_commands(Commands::new());
      (list, history, commands)
    });
    let add = move |item: &'static str| {
      history.apply(
        format!("Add {item}"),
        move || list.update(|l| l.push(item.to_owned())),
        move || {
          list.update(|l| {
            l.pop();
          })
        },
      );
    };
    assert!(!commands.is_enabled("edit.undo"), "nothing to undo yet");
    add("milk");
    add("eggs");
    assert_eq!(list.get(), ["milk", "eggs"]);
    assert_eq!(history.undo_name().as_deref(), Some("Add eggs"));

    assert!(commands.run("edit.undo"));
    assert_eq!(list.get(), ["milk"]);
    assert!(commands.is_enabled("edit.redo"));
    assert_eq!(history.redo_name().as_deref(), Some("Add eggs"));

    assert!(commands.run("edit.undo"));
    assert!(list.get().is_empty());
    assert!(!commands.run("edit.undo"), "disabled with nothing left");

    assert!(commands.run("edit.redo"));
    assert!(commands.run("edit.redo"));
    assert_eq!(list.get(), ["milk", "eggs"]);
    assert!(!commands.is_enabled("edit.redo"));

    // A new edit after an undo forgets what was undone.
    commands.run("edit.undo");
    add("bread");
    assert_eq!(list.get(), ["milk", "bread"]);
    assert!(!history.can_redo());
    history.clear();
    assert!(!history.can_undo());
  }
}
