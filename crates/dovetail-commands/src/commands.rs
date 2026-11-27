//! Commands: actions with a name and a shortcut, defined once and run from
//! menus, toolbar buttons, shortcuts and command search alike.

use std::rc::Rc;

use dovetail::{KeyEvent, Signal, create_signal};

use crate::shortcut::Shortcut;

/// One command, as a menu or command search shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandInfo {
  /// The id it is found by, such as `edit.undo`.
  pub id: String,
  /// The name shown, such as "Undo".
  pub name: String,
  /// Its shortcut now, if any.
  pub shortcut: Option<Shortcut>,
  /// The shortcut it was defined with, if any.
  pub default_shortcut: Option<Shortcut>,
  /// It can be run now.
  pub enabled: bool,
  /// Its icon, shown wherever the command is (M17).
  pub icon: Option<dovetail_image::Icon>,
  /// Whether it is on, for a toggle or a choice (M25), or `None` for a
  /// plain command.
  pub on: Option<bool>,
  /// The group a choice is one of (M25).
  pub group: Option<String>,
}

impl CommandInfo {
  /// A command with this id that does not exist: shown greyed out under
  /// its id.
  pub fn missing(id: &str) -> CommandInfo {
    CommandInfo {
      id: id.to_owned(),
      name: id.to_owned(),
      shortcut: None,
      default_shortcut: None,
      enabled: false,
      icon: None,
      on: None,
      group: None,
    }
  }
}

/// A shortcut that two commands would share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutConflict {
  /// The shortcut.
  pub shortcut: Shortcut,
  /// The command that has it.
  pub held_by: String,
  /// The command that asked for it.
  pub wanted_by: String,
}

impl std::fmt::Display for ShortcutConflict {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(
      f,
      "{} is already the shortcut for {}",
      self.shortcut, self.held_by
    )
  }
}

/// What kind of command it is (M25).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
  /// Runs, and has no state.
  Plain,
  /// On or off.
  Toggle,
  /// One of a group, of which one is on.
  Choice(String),
}

struct Entry {
  id: String,
  name: String,
  shortcut: Option<Shortcut>,
  default_shortcut: Option<Shortcut>,
  enabled: bool,
  enabled_when: Option<Rc<dyn Fn() -> bool>>,
  /// What it does. A toggle is given its new state; others are given
  /// true.
  run: Rc<dyn Fn(bool)>,
  icon: Option<dovetail_image::Icon>,
  kind: Kind,
  /// On, for a toggle or a choice.
  on: bool,
  /// Whether it is on, read where it is shown, instead of `on`.
  on_when: Option<Rc<dyn Fn() -> bool>>,
}

#[derive(Default)]
struct Registry {
  entries: Vec<Entry>,
  /// Shortcuts that could not be given because another command had them.
  conflicts: Vec<ShortcutConflict>,
}

impl Registry {
  fn get(
    &self,
    id: &str,
  ) -> Option<&Entry> {
    self.entries.iter().find(|e| e.id == id)
  }

  fn get_mut(
    &mut self,
    id: &str,
  ) -> Option<&mut Entry> {
    self.entries.iter_mut().find(|e| e.id == id)
  }

  fn holder(
    &self,
    shortcut: &Shortcut,
    except: &str,
  ) -> Option<&str> {
    self
      .entries
      .iter()
      .find(|e| e.id != except && e.shortcut.as_ref() == Some(shortcut))
      .map(|e| e.id.as_str())
  }
}

/// The application's commands. It is a small handle that can be copied
/// into closures; every copy refers to the same commands.
///
/// Components that show commands (menus, buttons, command search) are
/// built again when a command changes, such as when it is enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commands(Signal<RegistryCell>);

/// The registry, behind the signal. (A wrapper, so the handle can derive
/// its traits.)
pub struct RegistryCell(Registry);

impl std::fmt::Debug for RegistryCell {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(f, "Commands({})", self.0.entries.len())
  }
}

impl Default for Commands {
  fn default() -> Self {
    Self::new()
  }
}

impl Commands {
  /// No commands yet.
  pub fn new() -> Commands {
    Commands(create_signal(RegistryCell(Registry::default())))
  }

  /// Defines a command: its id, the name shown for it, a shortcut such as
  /// `"Mod+S"` (see [`Shortcut::parse`]), and what it does. Defining an id
  /// again replaces the command.
  ///
  /// If another command already has the shortcut, the new command gets
  /// none, and the clash is listed by [`Commands::conflicts`].
  ///
  /// # Panics
  ///
  /// If `shortcut` is not a shortcut. It is written in the program, so
  /// this is a mistake in the program.
  pub fn define(
    self,
    id: impl Into<String>,
    name: impl Into<String>,
    shortcut: Option<&str>,
    run: impl Fn() + 'static,
  ) -> Commands {
    self.define_kind(
      id.into(),
      name.into(),
      shortcut,
      Kind::Plain,
      Rc::new(move |_| run()),
    )
  }

  /// Defines a command that is on or off, as View > Grid is (M25). It
  /// starts off. Running it turns it over, then calls `run` with its new
  /// state. Menus show a tick while it is on, and buttons stay pressed.
  ///
  /// # Panics
  ///
  /// If `shortcut` is not a shortcut, as for [`Commands::define`].
  pub fn define_toggle(
    self,
    id: impl Into<String>,
    name: impl Into<String>,
    shortcut: Option<&str>,
    run: impl Fn(bool) + 'static,
  ) -> Commands {
    self.define_kind(
      id.into(),
      name.into(),
      shortcut,
      Kind::Toggle,
      Rc::new(run),
    )
  }

  /// Defines a command that is one of `group`, of which one is on, as the
  /// tools of a drawing program are (M25). The first command defined in a
  /// group starts on. Running one makes it the one that is on, then calls
  /// `run`. Menus show a dot by the one that is on.
  ///
  /// # Panics
  ///
  /// If `shortcut` is not a shortcut, as for [`Commands::define`].
  pub fn define_choice(
    self,
    id: impl Into<String>,
    group: impl Into<String>,
    name: impl Into<String>,
    shortcut: Option<&str>,
    run: impl Fn() + 'static,
  ) -> Commands {
    self.define_kind(
      id.into(),
      name.into(),
      shortcut,
      Kind::Choice(group.into()),
      Rc::new(move |_| run()),
    )
  }

  fn define_kind(
    self,
    id: String,
    name: String,
    shortcut: Option<&str>,
    kind: Kind,
    run: Rc<dyn Fn(bool)>,
  ) -> Commands {
    let default_shortcut = shortcut.map(|s| {
      Shortcut::parse(s)
        .unwrap_or_else(|e| panic!("the shortcut \"{s}\" for {id}: {e}"))
    });
    self.0.update(|RegistryCell(r)| {
      let mut shortcut = default_shortcut.clone();
      if let Some(s) = &shortcut
        && let Some(holder) = r.holder(s, &id)
      {
        r.conflicts.push(ShortcutConflict {
          shortcut: s.clone(),
          held_by: holder.to_owned(),
          wanted_by: id.clone(),
        });
        shortcut = None;
      }
      let entry = Entry {
        id: id.clone(),
        name,
        shortcut,
        default_shortcut,
        enabled: true,
        enabled_when: None,
        run,
        icon: None,
        on: false,
        on_when: None,
        kind,
      };
      let mut entry = entry;
      let old = r.entries.iter().position(|e| e.id == id);
      if let Some(i) = old {
        // Defined again, it keeps its state.
        entry.on = r.entries[i].on && r.entries[i].kind == entry.kind;
      } else if let Kind::Choice(group) = &entry.kind {
        // The first of a group is on.
        entry.on = !r
          .entries
          .iter()
          .any(|e| matches!(&e.kind, Kind::Choice(g) if g == group));
      }
      match old {
        Some(i) => r.entries[i] = entry,
        None => r.entries.push(entry),
      }
    });
    self
  }

  /// Gives a command an icon (M17), shown by its buttons, menu items and
  /// search results. Call it after [`Commands::define`], which starts a
  /// command with none.
  pub fn with_icon(
    self,
    id: &str,
    icon: dovetail_image::Icon,
  ) -> Commands {
    self.0.update(|RegistryCell(r)| {
      if let Some(e) = r.get_mut(id) {
        e.icon = Some(icon);
      }
    });
    self
  }

  /// Enables or disables a command.
  pub fn set_enabled(
    self,
    id: &str,
    enabled: bool,
  ) {
    self.0.update(|RegistryCell(r)| {
      if let Some(e) = r.get_mut(id) {
        e.enabled = enabled;
      }
    });
  }

  /// Makes a command enabled only while `rule` returns true. The rule is
  /// checked whenever the command is shown or run, and what it reads (such
  /// as the undo history) builds the showing components again when it
  /// changes.
  pub fn enabled_when(
    self,
    id: &str,
    rule: impl Fn() -> bool + 'static,
  ) -> Commands {
    let rule: Rc<dyn Fn() -> bool> = Rc::new(rule);
    self.0.update(|RegistryCell(r)| {
      if let Some(e) = r.get_mut(id) {
        e.enabled_when = Some(rule);
      }
    });
    self
  }

  /// True when the command exists and can be run now.
  pub fn is_enabled(
    self,
    id: &str,
  ) -> bool {
    let found = self.0.map(|RegistryCell(r)| {
      r.get(id)
        .map(|e| (e.enabled, e.enabled_when.clone()))
    });
    // The rule runs outside the registry, as it may read other state.
    match found {
      Some((true, Some(rule))) => rule(),
      Some((enabled, _)) => enabled,
      None => false,
    }
  }

  /// Runs a command, if it exists and is enabled. Returns whether it ran.
  pub fn run(
    self,
    id: &str,
  ) -> bool {
    if !self.is_enabled(id) {
      return false;
    }
    let found = self.0.map(|RegistryCell(r)| {
      r.get(id).map(|e| (e.run.clone(), e.kind.clone()))
    });
    let Some((run, kind)) = found else {
      return false;
    };
    let state = match kind {
      Kind::Plain => true,
      Kind::Toggle => {
        let on = !self.is_on(id).unwrap_or(false);
        self.set_on(id, on);
        on
      },
      Kind::Choice(_) => {
        self.set_on(id, true);
        true
      },
    };
    run(state);
    true
  }

  /// Turns a toggle on or off, or makes a choice the one of its group
  /// that is on (M25), without running it. A choice turned off leaves its
  /// group with none on. Plain commands have no state.
  pub fn set_on(
    self,
    id: &str,
    on: bool,
  ) {
    self.0.update(|RegistryCell(r)| {
      let Some(kind) = r.get(id).map(|e| e.kind.clone()) else {
        return;
      };
      match kind {
        Kind::Plain => {},
        Kind::Toggle => {
          if let Some(e) = r.get_mut(id) {
            e.on = on;
          }
        },
        Kind::Choice(group) => {
          for e in &mut r.entries {
            if e.id == id {
              e.on = on;
            } else if on && matches!(&e.kind, Kind::Choice(g) if *g == group) {
              e.on = false;
            }
          }
        },
      }
    });
  }

  /// Makes a toggle or choice on only while `rule` returns true (M25), so
  /// its state follows the application's own, as `enabled_when` does for
  /// being enabled. The rule is read wherever the command is shown.
  pub fn on_when(
    self,
    id: &str,
    rule: impl Fn() -> bool + 'static,
  ) -> Commands {
    let rule: Rc<dyn Fn() -> bool> = Rc::new(rule);
    self.0.update(|RegistryCell(r)| {
      if let Some(e) = r.get_mut(id) {
        e.on_when = Some(rule);
      }
    });
    self
  }

  /// Whether a toggle or choice is on (M25), or `None` for a plain
  /// command or one that does not exist.
  pub fn is_on(
    self,
    id: &str,
  ) -> Option<bool> {
    let found = self.0.map(|RegistryCell(r)| {
      r.get(id)
        .map(|e| (e.kind.clone(), e.on, e.on_when.clone()))
    })?;
    // The rule runs outside the registry, as it may read other state.
    match found {
      (Kind::Plain, ..) => None,
      (_, _, Some(rule)) => Some(rule()),
      (_, on, None) => Some(on),
    }
  }

  /// The command of `group` that is on (M25), if any.
  pub fn chosen(
    self,
    group: &str,
  ) -> Option<String> {
    self
      .group(group)
      .into_iter()
      .find(|id| self.is_on(id) == Some(true))
  }

  /// The commands of `group`, in the order they were defined (M25).
  pub fn group(
    self,
    group: &str,
  ) -> Vec<String> {
    self.0.map(|RegistryCell(r)| {
      r.entries
        .iter()
        .filter(|e| matches!(&e.kind, Kind::Choice(g) if g == group))
        .map(|e| e.id.clone())
        .collect()
    })
  }

  /// A command, as menus show it.
  pub fn info(
    self,
    id: &str,
  ) -> Option<CommandInfo> {
    let info = self.0.map(|RegistryCell(r)| {
      r.get(id).map(|e| CommandInfo {
        id: e.id.clone(),
        name: e.name.clone(),
        shortcut: e.shortcut.clone(),
        default_shortcut: e.default_shortcut.clone(),
        enabled: false,
        icon: e.icon.clone(),
        on: None,
        group: match &e.kind {
          Kind::Choice(g) => Some(g.clone()),
          _ => None,
        },
      })
    })?;
    Some(CommandInfo {
      enabled: self.is_enabled(id),
      on: self.is_on(id),
      ..info
    })
  }

  /// Every command, in the order they were defined.
  pub fn list(self) -> Vec<CommandInfo> {
    let ids: Vec<String> = self
      .0
      .map(|RegistryCell(r)| r.entries.iter().map(|e| e.id.clone()).collect());
    ids
      .iter()
      .filter_map(|id| self.info(id))
      .collect()
  }

  /// Gives a command a new shortcut, or none. A shortcut another command
  /// has is refused, and the clash is returned.
  pub fn set_shortcut(
    self,
    id: &str,
    shortcut: Option<Shortcut>,
  ) -> Result<(), ShortcutConflict> {
    let mut result = Ok(());
    self.0.update(|RegistryCell(r)| {
      if let Some(s) = &shortcut
        && let Some(holder) = r.holder(s, id)
      {
        result = Err(ShortcutConflict {
          shortcut: s.clone(),
          held_by: holder.to_owned(),
          wanted_by: id.to_owned(),
        });
        return;
      }
      if let Some(e) = r.get_mut(id) {
        e.shortcut = shortcut;
      }
      r.conflicts.retain(|c| c.wanted_by != id);
    });
    result
  }

  /// Gives a command back the shortcut it was defined with.
  pub fn reset_shortcut(
    self,
    id: &str,
  ) -> Result<(), ShortcutConflict> {
    let default = self.0.map(|RegistryCell(r)| {
      r.get(id).and_then(|e| e.default_shortcut.clone())
    });
    self.set_shortcut(id, default)
  }

  /// Shortcuts that commands were defined with but could not have,
  /// because another command had them first.
  pub fn conflicts(self) -> Vec<ShortcutConflict> {
    self.0.map(|RegistryCell(r)| r.conflicts.clone())
  }

  /// The command whose shortcut `event` is, if any.
  pub fn for_key(
    self,
    event: &KeyEvent,
  ) -> Option<String> {
    self.0.map(|RegistryCell(r)| {
      r.entries
        .iter()
        .find(|e| {
          e.shortcut
            .as_ref()
            .is_some_and(|s| s.matches(event))
        })
        .map(|e| e.id.clone())
    })
  }

  /// Runs the command whose shortcut `event` is, if it is enabled. Returns
  /// whether one ran.
  pub fn handle_key(
    self,
    event: &KeyEvent,
  ) -> bool {
    self
      .for_key(event)
      .is_some_and(|id| self.run(&id))
  }

  /// The commands whose names match `query`, best first: every character
  /// of the query in order, with whole words and the start of the name
  /// counting most. An empty query lists every command, in the order they
  /// were defined.
  pub fn search(
    self,
    query: &str,
  ) -> Vec<CommandInfo> {
    if query.trim().is_empty() {
      return self.list();
    }
    let mut found: Vec<(i32, usize, CommandInfo)> = self
      .list()
      .into_iter()
      .enumerate()
      .filter_map(|(i, c)| score(query, &c.name).map(|s| (s, i, c)))
      .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, c)| c).collect()
  }
}

/// How well `name` matches `query`, or `None` when it does not contain the
/// query's characters in order. Case is ignored.
fn score(
  query: &str,
  name: &str,
) -> Option<i32> {
  let name: Vec<char> = name.to_lowercase().chars().collect();
  let mut score = 0;
  let mut at = 0;
  let mut last: Option<usize> = None;
  for q in query
    .to_lowercase()
    .chars()
    .filter(|c| !c.is_whitespace())
  {
    let i = (at..name.len()).find(|&i| name[i] == q)?;
    score += 1;
    if i == 0 {
      score += 6;
    } else if !name[i - 1].is_alphanumeric() {
      score += 4;
    }
    if last.is_some_and(|l| l + 1 == i) {
      score += 3;
    }
    last = Some(i);
    at = i + 1;
  }
  Some(score - name.len() as i32 / 8)
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;
  use std::rc::Rc;

  use dovetail::{KeyEvent, Modifiers, NamedKey};

  use super::*;

  fn ctrl(c: char) -> KeyEvent {
    let m = if cfg!(target_os = "macos") {
      Modifiers { meta: true, ..Modifiers::default() }
    } else {
      Modifiers { control: true, ..Modifiers::default() }
    };
    let mut e = KeyEvent::character(c).with(m);
    e.text = None;
    e
  }

  #[test]
  fn commands_run_by_id_and_shortcut_only_when_enabled() {
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    let (_tree, commands) = crate::owned(move || {
      Commands::new()
        .define("count", "Count", Some("Mod+K"), move || c.set(c.get() + 1))
    });
    assert!(commands.run("count"));
    assert!(commands.handle_key(&ctrl('k')));
    assert!(!commands.handle_key(&ctrl('j')));
    assert_eq!(count.get(), 2);

    commands.set_enabled("count", false);
    assert!(!commands.is_enabled("count"));
    assert!(!commands.run("count"));
    assert!(
      !commands.handle_key(&ctrl('k')),
      "a disabled shortcut does nothing"
    );
    assert_eq!(count.get(), 2);
    assert!(!commands.run("missing"));
  }

  #[test]
  fn a_rule_enables_a_command_while_it_holds() {
    let (_tree, (open, commands)) = crate::owned(|| {
      let open = dovetail::create_signal(false);
      let commands = Commands::new()
        .define("save", "Save", Some("Mod+S"), || {})
        .enabled_when("save", move || open.get());
      (open, commands)
    });
    assert!(!commands.is_enabled("save"));
    open.set(true);
    assert!(commands.is_enabled("save"));
    assert!(commands.info("save").unwrap().enabled);
    // Disabled directly, the rule does not matter.
    commands.set_enabled("save", false);
    assert!(!commands.is_enabled("save"));
  }

  #[test]
  fn toggles_turn_over_and_choices_keep_one_on() {
    let seen = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let s = seen.clone();
    let (_tree, commands) = crate::owned(move || {
      let s2 = s.clone();
      Commands::new()
        .define("plain", "Plain", None, || {})
        .define_toggle("grid", "Grid", Some("Mod+G"), move |on| {
          s.borrow_mut().push(format!("grid {on}"))
        })
        .define_choice("pen", "tools", "Pen", Some("P"), move || {
          s2.borrow_mut().push("pen".into())
        })
        .define_choice("brush", "tools", "Brush", Some("B"), || {})
        .define_choice("eraser", "tools", "Eraser", Some("E"), || {})
    });
    assert_eq!(commands.is_on("plain"), None);
    assert_eq!(commands.is_on("grid"), Some(false));
    assert_eq!(
      commands.chosen("tools").as_deref(),
      Some("pen"),
      "the first"
    );
    assert_eq!(commands.group("tools"), ["pen", "brush", "eraser"]);

    assert!(commands.run("grid"));
    assert_eq!(commands.is_on("grid"), Some(true));
    assert!(commands.handle_key(&ctrl('g')));
    assert_eq!(commands.is_on("grid"), Some(false));
    assert_eq!(*seen.borrow(), ["grid true", "grid false"]);

    let b = KeyEvent::character('b');
    assert!(commands.handle_key(&b), "a letter key with no modifier");
    assert_eq!(commands.chosen("tools").as_deref(), Some("brush"));
    assert_eq!(commands.is_on("pen"), Some(false));
    let info = commands.info("brush").unwrap();
    assert_eq!(
      (info.on, info.group.as_deref()),
      (Some(true), Some("tools"))
    );
    assert_eq!(commands.info("grid").unwrap().group, None);

    commands.set_on("eraser", true);
    assert_eq!(commands.chosen("tools").as_deref(), Some("eraser"));
    commands.set_on("eraser", false);
    assert_eq!(commands.chosen("tools"), None);
    commands.run("pen");
    assert_eq!(seen.borrow().last().map(String::as_str), Some("pen"));

    // Defined again, a command keeps its state.
    commands.set_on("grid", true);
    let commands = commands.define_toggle("grid", "Grid", None, |_| {});
    assert_eq!(commands.is_on("grid"), Some(true));
  }

  #[test]
  fn a_rule_says_whether_a_toggle_is_on() {
    let (_tree, (wrap, commands)) = crate::owned(|| {
      let wrap = dovetail::create_signal(false);
      let commands = Commands::new()
        .define_toggle("wrap", "Wrap lines", None, move |on| wrap.set(on))
        .on_when("wrap", move || wrap.get());
      (wrap, commands)
    });
    assert_eq!(commands.is_on("wrap"), Some(false));
    wrap.set(true);
    assert_eq!(commands.is_on("wrap"), Some(true), "the rule follows");
    commands.run("wrap");
    assert!(!wrap.get(), "running turns it over from what the rule says");
    assert_eq!(commands.info("wrap").unwrap().on, Some(false));
  }

  #[test]
  fn shortcuts_can_be_changed_and_conflicts_are_reported() {
    let (_tree, commands) = crate::owned(|| {
      Commands::new()
        .define("a", "Alpha", Some("Mod+A"), || {})
        .define("b", "Beta", Some("Mod+B"), || {})
        // Defined with a shortcut that is taken: it gets none.
        .define("c", "Gamma", Some("Mod+A"), || {})
    });
    let conflicts = commands.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].held_by, "a");
    assert_eq!(conflicts[0].wanted_by, "c");
    assert_eq!(commands.info("c").unwrap().shortcut, None);

    let mod_b = Shortcut::parse("Mod+B").unwrap();
    let refused = commands
      .set_shortcut("a", Some(mod_b.clone()))
      .unwrap_err();
    assert_eq!(refused.held_by, "b");
    assert!(
      refused
        .to_string()
        .contains("is already the shortcut for b")
    );

    let mod_g = Shortcut::parse("Mod+G").unwrap();
    commands
      .set_shortcut("c", Some(mod_g.clone()))
      .unwrap();
    assert!(
      commands.conflicts().is_empty(),
      "c has a shortcut of its own now"
    );
    assert_eq!(commands.for_key(&ctrl('g')).as_deref(), Some("c"));
    commands.set_shortcut("b", None).unwrap();
    commands.set_shortcut("a", Some(mod_b)).unwrap();
    assert_eq!(commands.for_key(&ctrl('b')).as_deref(), Some("a"));
    commands.reset_shortcut("a").unwrap();
    assert_eq!(commands.for_key(&ctrl('a')).as_deref(), Some("a"));
    assert!(
      commands
        .for_key(&KeyEvent::named(NamedKey::F1))
        .is_none()
    );
  }

  #[test]
  fn search_finds_names_by_their_letters_in_order() {
    let (_tree, commands) = crate::owned(|| {
      Commands::new()
        .define("undo", "Undo", None, || {})
        .define("redo", "Redo", None, || {})
        .define("clear", "Clear the list", None, || {})
        .define("search", "Search commands", None, || {})
    });
    let names = |q: &str| -> Vec<String> {
      commands
        .search(q)
        .into_iter()
        .map(|c| c.name)
        .collect()
    };
    assert_eq!(names("und"), ["Undo"]);
    assert_eq!(names("do"), ["Undo", "Redo"]);
    assert_eq!(names("cl"), ["Clear the list"]);
    assert_eq!(names("tl"), ["Clear the list"], "the start of each word");
    assert_eq!(names("sc")[0], "Search commands");
    assert_eq!(
      names(""),
      [
        "Undo",
        "Redo",
        "Clear the list",
        "Search commands"
      ]
    );
    assert!(names("zzz").is_empty());
  }
}
