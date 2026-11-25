//! M06 "Done when" items 5 (Tab order) and 6 (keys go to the focused
//! element, and unhandled keys to its parents), with no window or GPU.

use std::cell::RefCell;
use std::rc::Rc;

use dovetail_core::{
  BuildContext, Callback, Component, Element, ElementKind, KeyEvent,
  KeyHandler, Modifiers, NamedKey, Size, Style, Tree, View,
};
use dovetail_layout::{BoxKey, FixedPitch, Layout};

/// Shows the view it is given.
struct Show;

impl Component for Show {
  type Props = View;

  fn build(
    view: &View,
    _: &mut BuildContext<'_>,
  ) -> View {
    view.clone()
  }
}

type Log = Rc<RefCell<Vec<String>>>;

fn sized(
  width: f32,
  height: f32,
) -> Style {
  Style {
    width: Size::Fixed(width),
    height: Size::Fixed(height),
    ..Style::default()
  }
}

/// A focusable element that logs presses, and keys when `takes` says so.
fn field(
  name: &'static str,
  log: &Log,
  takes: fn(&KeyEvent) -> bool,
) -> View {
  let (press_log, key_log) = (log.clone(), log.clone());
  Element::new(ElementKind::Box)
    .style(sized(50.0, 20.0))
    .focusable(true)
    .on_press(Callback::new(move |()| {
      press_log
        .borrow_mut()
        .push(format!("{name} pressed"))
    }))
    .on_key(KeyHandler::new(move |e| {
      let taken = takes(e);
      if taken {
        key_log
          .borrow_mut()
          .push(format!("{name} took {:?}", e.key));
      }
      taken
    }))
    .into()
}

/// A column of fields A, B (inside a group that takes Ctrl+S), C (not
/// focusable), D (clipped out of sight).
fn screen(log: &Log) -> (Tree, Layout) {
  let group_log = log.clone();
  let group: View = Element::new(ElementKind::Row)
    .on_key(KeyHandler::new(move |e| {
      let save = e.key == dovetail_core::KeyCode::Character("s".into())
        && e.modifiers.control;
      if save {
        group_log.borrow_mut().push("group saved".into());
      }
      save
    }))
    .children([field("B", log, |e| {
      e.is(NamedKey::ArrowLeft)
    })])
    .into();
  let plain: View = Element::new(ElementKind::Box)
    .style(sized(50.0, 20.0))
    .into();
  let hidden: View = Element::new(ElementKind::Box)
    .style(Style { clip: true, ..sized(0.0, 0.0) })
    .children([field("D", log, |_| false)])
    .into();
  let root: View = Element::new(ElementKind::Column)
    .children([
      field("A", log, |_| false),
      group,
      plain,
      hidden,
    ])
    .into();
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(root));
  let mut layout = Layout::new();
  layout.update(&tree, 300.0, 300.0, &FixedPitch::default());
  (tree, layout)
}

fn names(
  layout: &Layout,
  keys: &[&BoxKey],
) -> Vec<String> {
  let all = layout.focus_order();
  keys
    .iter()
    .map(|k| {
      ["A", "B"][all.iter().position(|a| a == k).expect("in order")].to_owned()
    })
    .collect()
}

#[test]
fn tab_order_follows_the_view_and_skips_what_cannot_be_seen() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let order = layout.focus_order();
  assert_eq!(
    order.len(),
    2,
    "A and B; C is not focusable and D is clipped away"
  );
  assert_eq!(names(&layout, &order), ["A", "B"]);
}

#[test]
fn tab_and_shift_tab_move_focus_and_wrap() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let a = layout.focus_next(None, false).expect("first");
  let b = layout
    .focus_next(Some(&a), false)
    .expect("second");
  assert_eq!(names(&layout, &[&a, &b]), ["A", "B"]);
  assert_eq!(
    layout.focus_next(Some(&b), false).as_ref(),
    Some(&a),
    "Tab wraps to the start"
  );
  assert_eq!(
    layout.focus_next(Some(&a), true).as_ref(),
    Some(&b),
    "Shift+Tab wraps to the end"
  );
  assert_eq!(
    layout.focus_next(None, true).as_ref(),
    Some(&b),
    "Shift+Tab from nothing goes to the last"
  );
}

#[test]
fn clicking_focuses_the_deepest_focusable_element() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let b = layout
    .focus_next(layout.focus_next(None, false).as_ref(), false)
    .unwrap();
  assert_eq!(
    layout.focusable_at(10.0, 30.0),
    Some(b),
    "B is at y 20 to 40"
  );
  assert_eq!(layout.focusable_at(10.0, 50.0), None, "C is not focusable");
}

#[test]
fn keys_go_to_the_focused_element_then_its_parents() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let a = layout.focus_next(None, false).unwrap();
  let b = layout.focus_next(Some(&a), false).unwrap();
  let ctrl = Modifiers { control: true, ..Modifiers::default() };

  assert!(
    layout.dispatch_key(Some(&b), &KeyEvent::named(NamedKey::ArrowLeft)),
    "B takes arrows"
  );
  assert!(
    layout.dispatch_key(Some(&b), &KeyEvent::character('s').with(ctrl)),
    "the group takes Ctrl+S from B"
  );
  assert!(
    !layout.dispatch_key(Some(&a), &KeyEvent::character('s').with(ctrl)),
    "A is not in the group"
  );
  assert!(
    !layout.dispatch_key(Some(&b), &KeyEvent::named(NamedKey::ArrowUp)),
    "nobody takes Up"
  );
  assert!(
    !layout.dispatch_key(None, &KeyEvent::named(NamedKey::Enter)),
    "no focus, no handler"
  );
  assert_eq!(*log.borrow(), ["B took Named(ArrowLeft)", "group saved"]);
}

#[test]
fn enter_and_space_press_the_focused_element() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let a = layout.focus_next(None, false).unwrap();
  assert!(layout.dispatch_key(Some(&a), &KeyEvent::named(NamedKey::Enter)));
  assert!(layout.dispatch_key(Some(&a), &KeyEvent::named(NamedKey::Space)));
  let shift = Modifiers { shift: true, ..Modifiers::default() };
  assert!(
    !layout
      .dispatch_key(Some(&a), &KeyEvent::named(NamedKey::Enter).with(shift)),
    "not with modifiers"
  );
  assert_eq!(*log.borrow(), ["A pressed", "A pressed"]);
}

#[test]
fn the_focus_ring_follows_the_element() {
  let log = Log::default();
  let (_tree, layout) = screen(&log);
  let a = layout.focus_next(None, false).unwrap();
  let (rect, radius) = layout.focus_ring(&a).expect("A is laid out");
  assert_eq!(
    (rect.x, rect.y, rect.width, rect.height, radius),
    (0.0, 0.0, 50.0, 20.0, 0.0)
  );
  assert!(layout.can_focus(&a));
}
