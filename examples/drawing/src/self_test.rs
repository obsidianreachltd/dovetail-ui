//! The self-test, in a real window (M20, `TestWindows`). Input goes
//! through the window's own test functions, so no key presses reach the
//! system. It checks:
//!
//! - a tool chosen with a click on the palette, and a shape drawn with it;
//! - tools chosen with their letter keys, and Shift with a letter going
//!   through a group;
//! - a group opened with a long press and with a right-click;
//! - letters typed into a text field, which leave the tool as it was;
//! - toggles from the View menu, with ticks, from the toolbar, pressed,
//!   and from a shortcut; snapping to the grid; undo;
//! - the dark theme's colours on the palette and the menu's ticks.

use std::process::ExitCode;
use std::time::Duration;

use dovetail::commands::LONG_PRESS;
use dovetail::prelude::*;
use dovetail::theme::ThemeData;
use dovetail::{KeyEvent, Modifiers, NamedKey, Role};
use dovetail_testing::windows::TestWindows;
use dovetail_testing::{role, text};

use dovetail::docking::Place;

use crate::{DOCK, GRID, STATE, State};

fn state() -> State {
  STATE.get().expect("the app is built")
}

/// The tool chosen now.
fn chosen() -> Option<String> {
  state().commands.chosen("tools")
}

/// Waits until `check` is true, then says it passed.
fn expect(
  t: &mut TestWindows,
  says: &str,
  mut check: impl FnMut(&mut TestWindows) -> bool,
) {
  t.wait_until(says, |t| check(t));
  println!("PASS  {says}");
}

fn steps(t: &mut TestWindows) {
  let w = t.open("Drawing self-test", (900.0, 640.0), ui! { crate::App {} });
  expect(t, "the palette, toolbar and canvas show", |t| {
    t.has(w, role(Role::Toolbar).label("Tools"))
      && t.has(w, role(Role::Canvas).label("Drawing"))
      && t.has(w, role(Role::Button).label("Grid"))
  });

  // A click on the palette, then a drag on the canvas.
  t.click(w, role(Role::RadioButton).label("Rectangle"));
  expect(t, "a click on the palette chooses the rectangle", |_| {
    chosen().as_deref() == Some("tool.rectangle")
  });
  let canvas = t.find(w, role(Role::Canvas)).rect();
  let from = (canvas.x + 60.0, canvas.y + 60.0);
  t.drag(w, from, (from.0 + 120.0, from.1 + 80.0));
  expect(t, "a drag on the canvas draws a rectangle", |t| {
    state().shapes.get().len() == 1
      && t.has(w, text("Untitled · Tool: Rectangle · 1 shape"))
  });

  // Letter keys, and Shift with a letter through a group.
  t.press(w, KeyEvent::character('p'));
  expect(t, "P chooses the pen", |_| {
    chosen().as_deref() == Some("tool.pen")
  });
  t.press(
    w,
    KeyEvent::character('P')
      .with(Modifiers { shift: true, ..Modifiers::default() }),
  );
  expect(
    t,
    "Shift+P goes on to the brush, which its button shows",
    |t| {
      chosen().as_deref() == Some("tool.brush")
        && t.has(w, role(Role::RadioButton).label("Brush"))
    },
  );

  // A long press opens the group.
  let at = t.pointer_down(w, role(Role::RadioButton).label("Brush"));
  t.pause(LONG_PRESS + Duration::from_millis(200));
  expect(t, "a long press opens the group", |t| {
    t.has(w, role(Role::Menu).label("Tools in this group"))
  });
  t.pointer_up(w, at);
  t.click(w, role(Role::MenuItemRadio).label("Pen"));
  expect(
    t,
    "choosing from the group chooses the pen and closes it",
    |t| {
      chosen().as_deref() == Some("tool.pen")
        && !t.has(w, role(Role::Menu).label("Tools in this group"))
    },
  );
  // And a right-click.
  t.right_click(w, role(Role::RadioButton).label("Rectangle"));
  expect(t, "a right-click opens the rectangle's group", |t| {
    t.has(w, role(Role::MenuItemRadio).label("Ellipse"))
  });
  t.click(w, role(Role::MenuItemRadio).label("Ellipse"));
  expect(t, "and the ellipse is chosen from it", |_| {
    chosen().as_deref() == Some("tool.ellipse")
  });

  // Letters in a text field are text.
  t.click(w, role(Role::TextInput).label("Title"));
  t.shortcut(w, 'a');
  t.type_text(w, "Plan");
  expect(
    t,
    "letters typed in the title stay there, and the tool stays",
    |t| {
      state().title.get() == "Plan"
        && chosen().as_deref() == Some("tool.ellipse")
        && t.has(w, text("Plan · Tool: Ellipse · 1 shape"))
    },
  );

  // Toggles: the View menu, ticked; the toolbar, pressed; a shortcut.
  t.click(w, role(Role::MenuItem).label("View"));
  expect(t, "the View menu shows Grid without a tick", |t| {
    t.try_find(w, role(Role::MenuItemCheckBox).label("Grid"))
      .is_ok_and(|f| f.is_on() == Some(false))
  });
  t.click(w, role(Role::MenuItemCheckBox).label("Grid"));
  expect(
    t,
    "Grid turns on, and the toolbar's button is pressed",
    |t| {
      state().grid.get()
        && t
          .try_find(w, role(Role::Button).label("Grid"))
          .is_ok_and(|f| f.is_on() == Some(true))
    },
  );
  t.click(w, role(Role::MenuItem).label("View"));
  expect(t, "the View menu ticks Grid", |t| {
    t.try_find(w, role(Role::MenuItemCheckBox).label("Grid"))
      .is_ok_and(|f| f.is_on() == Some(true))
  });
  t.key(w, NamedKey::Escape);
  t.click(w, role(Role::Button).label("Snap to grid"));
  expect(t, "the toolbar turns snapping on", |_| state().snap.get());

  // A snapped ellipse.
  let canvas = t.find(w, role(Role::Canvas)).rect();
  let from = (canvas.x + 207.0, canvas.y + 133.0);
  t.drag(w, from, (from.0 + 93.0, from.1 + 52.0));
  expect(
    t,
    "a drag with snapping puts the ellipse on the grid",
    |_| {
      let shapes = state().shapes.get();
      shapes.len() == 2 && {
        let s = &shapes[1];
        [s.from.0, s.from.1, s.to.0, s.to.1]
          .iter()
          .all(|v| (v / GRID).fract().abs() < 1e-3)
      }
    },
  );
  t.shortcut(w, 'g');
  expect(t, "Ctrl+G turns the grid off again", |_| {
    !state().grid.get()
  });
  t.shortcut(w, 'z');
  expect(t, "undo takes the ellipse away", |_| {
    state().shapes.get().len() == 1
  });

  // The dark theme: the palette's chosen tool and the menu follow it.
  t.click(w, role(Role::MenuItem).label("View"));
  t.click(w, role(Role::MenuItemCheckBox).label("Dark theme"));
  expect(t, "the dark theme is on", |_| state().dark.get());
  let pressed = ThemeData::dark().group_look().pressed;
  expect(
    t,
    "the chosen tool takes the dark theme's pressed colour",
    |t| {
      t.with(move |cx| {
        let id = cx.windows()[0];
        cx.ui_boxes(id).iter().any(|b| match &b.content {
          dovetail::layout::Content::Element(e) => {
            e.role == Role::RadioButton
              && e.label.as_deref() == Some("Ellipse")
              && e.style.background == pressed
          },
          _ => false,
        })
      })
    },
  );
  t.click(w, role(Role::MenuItem).label("View"));
  expect(t, "the View menu ticks the dark theme", |t| {
    t.try_find(w, role(Role::MenuItemCheckBox).label("Dark theme"))
      .is_ok_and(|f| f.is_on() == Some(true))
  });
  t.key(w, NamedKey::Escape);

  // The palette floats in a window of its own, and still works there.
  t.with(|_| {
    if let Some(dock) = DOCK.get() {
      dock.move_panel("tools", Place::Float(None));
    }
  });
  t.wait_until("the palette's window", |t| t.windows().len() == 2);
  let float = t
    .windows()
    .into_iter()
    .find(|f| *f != w)
    .expect("the floating window");
  expect(t, "the palette shows in its floating window", |t| {
    t.has(float, role(Role::RadioButton).label("Select"))
  });
  t.click(float, role(Role::RadioButton).label("Line"));
  expect(
    t,
    "a click in the floating palette chooses the line",
    |_| chosen().as_deref() == Some("tool.line"),
  );
  let at = t.pointer_down(float, role(Role::RadioButton).label("Pen"));
  t.pause(LONG_PRESS + Duration::from_millis(200));
  t.pointer_up(float, at);
  expect(
    t,
    "a long press opens a group in the floating palette",
    |t| t.has(float, role(Role::MenuItemRadio).label("Brush")),
  );
  t.click(float, role(Role::MenuItemRadio).label("Brush"));
  expect(t, "and the brush is chosen from it", |_| {
    chosen().as_deref() == Some("tool.brush")
  });
  t.close_all();
}

pub fn run() -> ExitCode {
  let mut t = TestWindows::new();
  // A failed check panics with what it waited for, and the program fails.
  steps(&mut t);
  println!("PASS  the drawing tool works with the pointer and the keyboard");
  ExitCode::SUCCESS
}
