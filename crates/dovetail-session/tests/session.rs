//! A session runs a window's input with no window or GPU (D0027).

use std::cell::Cell;
use std::rc::Rc;

use dovetail_core::{
  BuildContext, Callback, Component, Element, ElementKind, KeyEvent, Modifiers,
  NamedKey, PointerButton, Role, View,
};
use dovetail_layout::FixedPitch;
use dovetail_session::{AccessId, AccessKind, Session, Settings};

/// Shows a fixed view.
struct Show;

#[derive(PartialEq)]
struct ShowProps {
  view: View,
}

impl Component for Show {
  type Props = ShowProps;

  fn build(
    props: &ShowProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    props.view.clone()
  }
}

/// A focusable box showing `text` that counts its presses.
fn button(
  text: &str,
  presses: &Rc<Cell<u32>>,
) -> View {
  let presses = Rc::clone(presses);
  Element::new(ElementKind::Box)
    .focusable(true)
    .on_press(Callback::new(move |()| presses.set(presses.get() + 1)))
    .children([View::text(text)])
    .into()
}

fn session(view: View) -> Session {
  let mut session = Session::new(
    "test",
    View::component::<Show>(ShowProps { view }),
    None,
    Settings::default(),
    Box::new(FixedPitch::default()),
  );
  session.update(Some((200.0, 100.0)));
  session
}

#[test]
fn a_click_focuses_and_presses_what_it_lands_on() {
  let first = Rc::new(Cell::new(0));
  let second = Rc::new(Cell::new(0));
  let mut s = session(
    Element::new(ElementKind::Column)
      .children([
        button("One", &first),
        button("Two", &second),
      ])
      .into(),
  );
  let none = Modifiers::default();
  s.set_cursor((2.0, 2.0));
  s.pointer_moved(none);
  s.press(none, PointerButton::Primary);
  s.release(none, PointerButton::Primary);
  s.update(Some((200.0, 100.0)));
  assert_eq!((first.get(), second.get()), (1, 0));
  assert!(s.focus().is_some(), "the click focused the box");
  // A click does not show the focus ring; Tab does.
  assert!(s.focus_ring().is_none());

  // Tab moves focus to the second box, and Enter presses it.
  assert!(s.key(&KeyEvent::named(NamedKey::Tab)));
  assert!(s.focus_ring().is_some());
  assert!(s.key(&KeyEvent::named(NamedKey::Enter)));
  assert_eq!((first.get(), second.get()), (1, 1));
}

#[test]
fn a_release_somewhere_else_does_not_press() {
  let presses = Rc::new(Cell::new(0));
  // In a column, the box is one line high, at the top.
  let mut s = session(
    Element::new(ElementKind::Column)
      .children([button("One", &presses)])
      .into(),
  );
  let none = Modifiers::default();
  s.set_cursor((2.0, 2.0));
  s.press(none, PointerButton::Primary);
  s.set_cursor((2.0, 90.0));
  s.pointer_moved(none);
  s.release(none, PointerButton::Primary);
  assert_eq!(presses.get(), 0);
}

#[test]
fn the_accessibility_tree_names_a_button_by_its_text() {
  let presses = Rc::new(Cell::new(0));
  let p = Rc::clone(&presses);
  let s = session(
    Element::new(ElementKind::Column)
      .children([Element::new(ElementKind::Box)
        .role(Role::Button)
        .focusable(true)
        .on_press(Callback::new(move |()| p.set(p.get() + 1)))
        .children([View::text("Save")])
        .into()])
      .into(),
  );
  let tree = s.access_tree();
  assert_eq!(tree.root().kind, AccessKind::Window);
  assert_eq!(tree.root().label.as_deref(), Some("test"));
  let buttons: Vec<_> = tree
    .nodes
    .iter()
    .filter(|n| n.role() == Some(Role::Button))
    .collect();
  assert_eq!(buttons.len(), 1);
  assert_eq!(buttons[0].label.as_deref(), Some("Save"));
  assert!(buttons[0].actions.click && buttons[0].actions.focus);
  // Its text names it, so it is not read again as a label of its own.
  assert!(buttons[0].children.is_empty());
  assert_eq!(tree.focus, AccessId::ROOT);
}
