//! A property grid for inspectors (M16): names and values in two columns,
//! in sections that open and close, with every change undoable (D0022).

use std::fmt;
use std::rc::Rc;

use dovetail::{
  Align, Callback, Edges, Element, ElementKind, KeyHandler, NamedKey, Role,
  Size, Style, View, use_state,
};
use dovetail_commands::History;
use dovetail_controls::{
  CheckBox, Collapsible, NumberField, Select, TextField,
};
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

/// A property's value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
  /// Text, edited in a text field.
  Text(String),
  /// A number, edited in a number field.
  Number(f32),
  /// On or off, edited with a check box.
  Bool(bool),
  /// One of a list of options, by its position, edited with a drop-down.
  Choice(usize),
}

/// How a property is edited.
#[derive(Clone, Debug, PartialEq)]
enum Editor {
  Text,
  Number {
    min: f32,
    max: f32,
    step: f32,
    decimals: usize,
  },
  Bool,
  Choice(Vec<String>),
}

/// One property: a name, a value, and how to change it.
#[derive(Clone)]
pub struct Property {
  name: String,
  value: PropertyValue,
  editor: Editor,
  set: Rc<dyn Fn(PropertyValue)>,
  enabled: bool,
}

impl PartialEq for Property {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.name == other.name
      && self.value == other.value
      && self.editor == other.editor
      && self.enabled == other.enabled
      && Rc::ptr_eq(&self.set, &other.set)
  }
}

impl fmt::Debug for Property {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.debug_struct("Property")
      .field("name", &self.name)
      .field("value", &self.value)
      .finish_non_exhaustive()
  }
}

impl Property {
  fn new(
    name: impl Into<String>,
    value: PropertyValue,
    editor: Editor,
    set: Rc<dyn Fn(PropertyValue)>,
  ) -> Property {
    Property {
      name: name.into(),
      value,
      editor,
      set,
      enabled: true,
    }
  }

  /// A text property. `set` is called with the new text.
  pub fn text(
    name: impl Into<String>,
    value: impl Into<String>,
    set: impl Fn(String) + 'static,
  ) -> Property {
    Property::new(
      name,
      PropertyValue::Text(value.into()),
      Editor::Text,
      Rc::new(move |v| {
        if let PropertyValue::Text(s) = v {
          set(s);
        }
      }),
    )
  }

  /// A number from `min` to `max`, stepped by `step` and shown with
  /// `decimals` places.
  pub fn number(
    name: impl Into<String>,
    value: f32,
    (min, max, step): (f32, f32, f32),
    decimals: usize,
    set: impl Fn(f32) + 'static,
  ) -> Property {
    Property::new(
      name,
      PropertyValue::Number(value),
      Editor::Number { min, max, step, decimals },
      Rc::new(move |v| {
        if let PropertyValue::Number(n) = v {
          set(n);
        }
      }),
    )
  }

  /// An on or off property.
  pub fn check(
    name: impl Into<String>,
    value: bool,
    set: impl Fn(bool) + 'static,
  ) -> Property {
    Property::new(
      name,
      PropertyValue::Bool(value),
      Editor::Bool,
      Rc::new(move |v| {
        if let PropertyValue::Bool(b) = v {
          set(b);
        }
      }),
    )
  }

  /// One of `options`, by its position.
  pub fn choice(
    name: impl Into<String>,
    options: Vec<String>,
    value: usize,
    set: impl Fn(usize) + 'static,
  ) -> Property {
    Property::new(
      name,
      PropertyValue::Choice(value),
      Editor::Choice(options),
      Rc::new(move |v| {
        if let PropertyValue::Choice(i) = v {
          set(i);
        }
      }),
    )
  }

  /// The same property, which cannot be changed now.
  pub fn disabled(mut self) -> Property {
    self.enabled = false;
    self
  }

  /// The property's name.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// The property's value.
  pub fn value(&self) -> &PropertyValue {
    &self.value
  }
}

/// A titled group of properties, which opens and closes.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySection {
  /// The title.
  pub title: String,
  /// The properties, in order.
  pub properties: Vec<Property>,
}

impl PropertySection {
  /// A section with this title and these properties.
  pub fn new(
    title: impl Into<String>,
    properties: Vec<Property>,
  ) -> PropertySection {
    PropertySection { title: title.into(), properties }
  }
}

/// Records a change from `old` to `new` in `history`, which makes it.
fn record(
  history: History,
  property: &Property,
  old: PropertyValue,
  new: PropertyValue,
) {
  if old == new {
    return;
  }
  let (redo, undo) = (property.set.clone(), property.set.clone());
  history.apply(
    format!("Change {}", property.name),
    move || redo(new.clone()),
    move || undo(old.clone()),
  );
}

/// Names and values in two columns, grouped in sections that open and
/// close. Values are edited with text fields, number fields, check boxes
/// and drop-downs, and each change is done through `history`, so Undo
/// takes it back (D0022).
///
/// A check box or drop-down change is one step. Typing in a text or number
/// field changes the value as it goes, and is kept as one step when Enter
/// is pressed, focus leaves the field, or another property is changed or
/// focused.
#[component]
pub fn PropertyGrid(
  sections: Vec<PropertySection>,
  history: History,
  #[prop(default = 120.0)] name_width: f32,
  #[prop(default = 200.0)] value_width: f32,
  #[prop(into, default = "Properties")] label: String,
) -> View {
  let t = theme();
  // Sections closed by the user, by title.
  let closed = use_state(Vec::<String>::new);
  // A text or number edit under way: the property's section and position,
  // and its value before the edit.
  let editing = use_state(|| None::<(usize, usize, PropertyValue)>);
  // Ends the edit under way, keeping it as one step.
  let finish: Finish = {
    let sections: Rc<[PropertySection]> = sections.clone().into();
    Rc::new(move || {
      if let Some((s, i, old)) = editing.get() {
        editing.set(None);
        if let Some(p) = sections.get(s).and_then(|x| x.properties.get(i)) {
          record(history, p, old, p.value.clone());
        }
      }
    })
  };

  let groups: Vec<View> = sections
    .iter()
    .enumerate()
    .map(|(si, section)| {
      let open = !closed.map(|c| c.contains(&section.title));
      let rows: Vec<View> = section
        .properties
        .iter()
        .enumerate()
        .map(|(pi, p)| {
          property_row(
            t.clone(),
            Edit { history, editing, finish: finish.clone() },
            (si, pi),
            p,
            (name_width, value_width),
          )
        })
        .collect();
      let title = section.title.clone();
      ui! {
        Collapsible {
          title: section.title.clone(),
          open: open,
          on_toggle: move |now: bool| closed.update(|c| {
            c.retain(|x| *x != title);
            if !now {
              c.push(title.clone());
            }
          }),
          {
            View::from(Element::new(ElementKind::Column)
            .style(Style {
              gap: t.spacing.of(1.0),
              padding: Edges { left: t.spacing.of(4.0), ..Edges::default() },
              ..Style::default()
            })
            .role(Role::Group)
            .label(section.title.clone())
            .children(rows)
            )
          }
        }
      }
    })
    .collect();
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(1.0),
      ..Style::default()
    })
    .role(Role::Group)
    .label(label)
    .children(groups)
    .into()
}

type Finish = Rc<dyn Fn()>;

/// What every row needs to edit through the history.
struct Edit {
  history: History,
  editing: dovetail::Signal<Option<(usize, usize, PropertyValue)>>,
  finish: Finish,
}

/// One property: its name, and its editor.
fn property_row(
  t: dovetail_theme::ThemeData,
  edit: Edit,
  at: (usize, usize),
  p: &Property,
  (name_width, value_width): (f32, f32),
) -> View {
  let Edit { history, editing, finish } = edit;
  // True when the edit under way is another property's.
  let other = move || {
    editing
      .get()
      .is_some_and(|(s, i, _)| (s, i) != at)
  };
  let name = p.name.clone();
  let enabled = p.enabled;
  // A text or number change: kept as one step until the edit ends.
  let live = {
    let p = p.clone();
    let finish = finish.clone();
    move |new: PropertyValue| {
      if other() {
        finish();
      }
      if editing.get().is_none() {
        editing.set(Some((at.0, at.1, p.value.clone())));
      }
      (p.set)(new);
    }
  };
  // A check box or drop-down change: one step, after any edit under way.
  let at_once = {
    let p = p.clone();
    let finish = finish.clone();
    move |old: PropertyValue, new: PropertyValue| {
      finish();
      record(history, &p, old, new);
    }
  };
  let editor: View = match (&p.editor, &p.value) {
    (Editor::Text, PropertyValue::Text(value)) => {
      let live = live.clone();
      ui! {
        TextField {
          value: value.clone(),
          on_change: move |s: String| live(PropertyValue::Text(s)),
          width: value_width,
          label: name.clone(),
        }
      }
    },
    (Editor::Number { min, max, step, decimals }, PropertyValue::Number(v)) => {
      let live = live.clone();
      ui! {
        NumberField {
          value: *v,
          on_change: move |n: f32| live(PropertyValue::Number(n)),
          min: *min,
          max: *max,
          step: *step,
          decimals: *decimals,
          width: value_width,
          label: name.clone(),
          enabled: enabled,
        }
      }
    },
    (Editor::Bool, PropertyValue::Bool(on)) => {
      let at_once = at_once.clone();
      let on = *on;
      ui! {
        CheckBox {
          checked: on,
          on_change: move |now: bool| {
            at_once(PropertyValue::Bool(on), PropertyValue::Bool(now))
          },
          label: name.clone(),
          enabled: enabled,
        }
      }
    },
    (Editor::Choice(options), PropertyValue::Choice(i)) => {
      let at_once = at_once.clone();
      let i = *i;
      ui! {
        Select {
          options: options.clone(),
          selected: Some(i),
          on_change: move |now: usize| {
            at_once(PropertyValue::Choice(i), PropertyValue::Choice(now))
          },
          label: name.clone(),
          width: value_width,
          enabled: enabled,
        }
      }
    },
    _ => View::Empty,
  };
  let (finish_on_key, finish_on_focus) = (finish.clone(), finish);
  Element::new(ElementKind::Row)
    .style(Style {
      align: Align::Center,
      gap: t.spacing.of(2.0),
      ..Style::default()
    })
    // Enter ends a text or number edit; Tab and clicks elsewhere end it
    // when focus leaves.
    .on_key(KeyHandler::new(move |event| {
      if event.is(NamedKey::Enter) {
        finish_on_key();
      }
      false
    }))
    .on_focus_within(Callback::new(move |inside: bool| {
      // Focus leaving this property, or coming to another, ends the
      // edit under way.
      if inside == other() {
        finish_on_focus();
      }
    }))
    .children([
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(name_width),
          text_color: Some(t.colors.on_surface_variant),
          ..Style::default()
        })
        .role(Role::Label)
        .children([View::text(p.name.clone())])
        .into(),
      editor,
    ])
    .into()
}
