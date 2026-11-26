//! M18 criterion 2: a component that reads a system setting is built
//! again when it changes, and components that do not read it are not.

use std::cell::RefCell;
use std::time::Duration;

use dovetail_core::{
  BuildContext, Component, SystemSettings, Tree, View, caret_blink,
  reduce_motion, set_system_settings, system_settings, text_scale,
};

thread_local! {
  /// The name of each component, each time it builds.
  static BUILT: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn built() -> Vec<&'static str> {
  let mut names = BUILT.with_borrow_mut(std::mem::take);
  names.sort_unstable();
  names
}

/// What a [`Reader`] reads while it builds.
#[derive(Clone, Copy, PartialEq)]
enum Reads {
  Motion,
  Scale,
  All,
  Nothing,
}

/// A component that reads one setting, all of them, or none.
struct Reader;

#[derive(PartialEq)]
struct ReaderProps {
  name: &'static str,
  reads: Reads,
}

impl Component for Reader {
  type Props = ReaderProps;

  fn build(
    props: &ReaderProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    BUILT.with_borrow_mut(|b| b.push(props.name));
    match props.reads {
      Reads::Motion => {
        reduce_motion();
      },
      Reads::Scale => {
        text_scale();
      },
      Reads::All => {
        system_settings();
      },
      Reads::Nothing => {},
    }
    View::text(props.name)
  }
}

fn reader(
  name: &'static str,
  reads: Reads,
) -> View {
  View::component::<Reader>(ReaderProps { name, reads })
}

/// The root: four readers side by side.
struct Window;

impl Component for Window {
  type Props = ();

  fn build(
    _: &(),
    _: &mut BuildContext<'_>,
  ) -> View {
    View::fragment([
      reader("motion", Reads::Motion),
      reader("scale", Reads::Scale),
      reader("all", Reads::All),
      reader("none", Reads::Nothing),
    ])
  }
}

#[test]
fn only_the_readers_of_a_changed_setting_are_built_again() {
  let start = SystemSettings::default();
  set_system_settings(start);
  let mut tree = Tree::new();
  tree.mount(View::component::<Window>(()));
  assert_eq!(built(), ["all", "motion", "none", "scale"]);

  // Less motion: its reader, and the one that reads everything.
  let mut settings = SystemSettings { reduce_motion: true, ..start };
  set_system_settings(settings);
  tree.update();
  assert_eq!(built(), ["all", "motion"]);

  // The text scale: its reader, and the one that reads everything.
  settings.text_scale = 1.5;
  set_system_settings(settings);
  tree.update();
  assert_eq!(built(), ["all", "scale"]);

  // A setting no one but "all" reads.
  settings.caret_blink = None;
  set_system_settings(settings);
  tree.update();
  assert_eq!(built(), ["all"]);
  assert_eq!(caret_blink(), None);

  // The same settings again change nothing.
  set_system_settings(settings);
  tree.update();
  assert_eq!(built(), Vec::<&str>::new());

  // Readers stay readers after being built again.
  settings.reduce_motion = false;
  settings.double_click_time = Duration::from_millis(300);
  set_system_settings(settings);
  tree.update();
  assert_eq!(built(), ["all", "motion"]);

  set_system_settings(start);
}
