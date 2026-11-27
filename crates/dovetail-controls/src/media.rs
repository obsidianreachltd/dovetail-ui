//! Images and icons (M17): `Icon`, `IconButton` and `Image`, which loads a
//! file off the UI thread with a placeholder until it is ready, shares it
//! with other `Image`s that show it, and plays animations (D0024).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use dovetail::{
  Align, Callback, Color, Edges, Element, ElementKind, Fit, Role, Size, Style,
  Task, View, use_state,
};
use dovetail_image::{Animation, Bitmap, Icon as IconData, icons};
use dovetail_macros::component;
use dovetail_theme::theme;

/// An icon, in the colour of the text around it unless given one, and as
/// tall as that text makes it unless given a size.
///
/// With a `label` it is an image for assistive tools; without one it is
/// decoration, which they skip, as an icon beside a button's text should
/// be.
#[component]
pub fn Icon(
  icon: IconData,
  #[prop(into, default)] size: Option<f32>,
  #[prop(into, default)] color: Option<Color>,
  #[prop(into, default = "")] label: String,
) -> View {
  let mut picture = icon.picture().label(label);
  if let Some(size) = size {
    picture = picture.size(size);
  }
  if let Some(color) = color {
    picture = picture.color(color);
  }
  picture.into()
}

/// A button that shows only an icon. `label` names it for assistive tools
/// and is shown as a tooltip by applications that want one.
#[component]
pub fn IconButton(
  icon: IconData,
  #[prop(into)] label: String,
  #[prop(default = Callback::new(|_: ()| {}))] on_click: Callback<()>,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let (off_background, off_foreground) = t.disabled_look();
  let pad = t.spacing.of(1.5);
  let button = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(pad),
      radius: 4.0,
      background: if enabled {
        Color::TRANSPARENT
      } else {
        off_background
      },
      text_color: Some(if enabled {
        t.colors.on_surface
      } else {
        off_foreground
      }),
      ..Style::default()
    })
    .role(Role::Button)
    .label(label.clone())
    .children([icon.picture().label(label).into()]);
  if enabled {
    button.focusable(true).on_press(on_click).into()
  } else {
    button.disabled(true).into()
  }
}

/// Where an [`Image`] comes from.
#[derive(Clone)]
pub enum ImageSource {
  /// A PNG, JPEG or GIF file, read and decoded off the UI thread.
  File(PathBuf),
  /// A PNG, JPEG or GIF file's bytes, decoded off the UI thread.
  Bytes(Arc<[u8]>),
  /// An image already decoded.
  Bitmap(Bitmap),
  /// An animation already decoded.
  Animation(Animation),
}

impl PartialEq for ImageSource {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    match (self, other) {
      (ImageSource::File(a), ImageSource::File(b)) => a == b,
      (ImageSource::Bytes(a), ImageSource::Bytes(b)) => Arc::ptr_eq(a, b),
      (ImageSource::Bitmap(a), ImageSource::Bitmap(b)) => a == b,
      (ImageSource::Animation(a), ImageSource::Animation(b)) => a == b,
      _ => false,
    }
  }
}

impl std::fmt::Debug for ImageSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    match self {
      ImageSource::File(p) => write!(f, "File({})", p.display()),
      ImageSource::Bytes(b) => write!(f, "Bytes({} bytes)", b.len()),
      ImageSource::Bitmap(b) => write!(f, "{b:?}"),
      ImageSource::Animation(a) => {
        write!(f, "Animation({} frames)", a.frames().len())
      },
    }
  }
}

impl From<Bitmap> for ImageSource {
  fn from(b: Bitmap) -> Self {
    ImageSource::Bitmap(b)
  }
}

impl From<Animation> for ImageSource {
  fn from(a: Animation) -> Self {
    ImageSource::Animation(a)
  }
}

impl From<PathBuf> for ImageSource {
  fn from(p: PathBuf) -> Self {
    ImageSource::File(p)
  }
}

impl From<&std::path::Path> for ImageSource {
  fn from(p: &std::path::Path) -> Self {
    ImageSource::File(p.to_path_buf())
  }
}

impl From<&str> for ImageSource {
  fn from(p: &str) -> Self {
    ImageSource::File(PathBuf::from(p))
  }
}

/// Where an image's loading has got to.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageState {
  /// Being read and decoded.
  Loading,
  /// Ready: the image, or an animation's first frame.
  Ready(Bitmap),
  /// It could not be read: why.
  Failed(String),
}

/// What files and bytes were decoded to, shared by every `Image` that shows
/// them (D0024).
mod cache {
  use std::cell::{Cell, RefCell};
  use std::collections::HashMap;
  use std::path::PathBuf;
  use std::sync::{Arc, Weak};
  use std::time::SystemTime;

  use dovetail::{Callback, TaskEnd, spawn_task};
  use dovetail_image::{Animation, WeakAnimation};

  use super::ImageSource;

  /// What tells one file or set of bytes from another.
  #[derive(Clone, PartialEq, Eq, Hash)]
  enum Key {
    /// A file, with its length and when it last changed, so a changed file
    /// is read again.
    File(PathBuf, u64, Option<SystemTime>),
    /// Bytes, by where they are.
    Bytes(usize, usize),
  }

  type Done = Callback<Result<Animation, String>>;

  enum Entry {
    /// Being decoded: who to tell.
    Loading(Vec<Done>),
    /// Decoded, while something keeps it; for bytes, while they are the
    /// same bytes.
    Ready(WeakAnimation, Option<Weak<[u8]>>),
  }

  thread_local! {
    static CACHE: RefCell<HashMap<Key, Entry>> = RefCell::new(HashMap::new());
    static DECODES: Cell<u64> = const { Cell::new(0) };
  }

  fn key(source: &ImageSource) -> Option<Key> {
    match source {
      ImageSource::File(path) => {
        let meta = std::fs::metadata(path).ok()?;
        Some(Key::File(path.clone(), meta.len(), meta.modified().ok()))
      },
      ImageSource::Bytes(b) => {
        Some(Key::Bytes(Arc::as_ptr(b).cast::<u8>() as usize, b.len()))
      },
      _ => None,
    }
  }

  /// How many files and bytes have been decoded on this thread.
  pub fn decodes() -> u64 {
    DECODES.get()
  }

  /// Decodes `source` (a file or bytes) off the UI thread and calls `done`,
  /// or calls it at once with what is already decoded. Several asking for
  /// the same source while it is decoded share one task.
  pub fn load(
    source: &ImageSource,
    done: Done,
  ) {
    let key = key(source);
    if let Some(k) = &key {
      let hit = CACHE.with_borrow_mut(|c| match c.get_mut(k) {
        Some(Entry::Loading(waiting)) => {
          waiting.push(done.clone());
          Some(None)
        },
        Some(Entry::Ready(weak, bytes)) => {
          let same_bytes = match (bytes, source) {
            (Some(b), ImageSource::Bytes(now)) => {
              b.upgrade().is_some_and(|b| Arc::ptr_eq(&b, now))
            },
            _ => true,
          };
          match weak.upgrade().filter(|_| same_bytes) {
            Some(a) => Some(Some(a)),
            None => {
              c.remove(k);
              None
            },
          }
        },
        None => None,
      });
      match hit {
        Some(Some(animation)) => {
          done.call(Ok(animation));
          return;
        },
        Some(None) => return,
        None => {},
      }
      CACHE.with_borrow_mut(|c| {
        c.insert(k.clone(), Entry::Loading(vec![done.clone()]));
      });
    }
    DECODES.set(DECODES.get() + 1);
    let wanted = source.clone();
    let bytes = match source {
      ImageSource::Bytes(b) => Some(Arc::downgrade(b)),
      _ => None,
    };
    spawn_task(
      move |_| {
        Some(match &wanted {
          ImageSource::File(path) => Animation::open(path),
          ImageSource::Bytes(bytes) => Animation::decode(bytes),
          ImageSource::Bitmap(b) => Ok(Animation::still(b.clone())),
          ImageSource::Animation(a) => Ok(a.clone()),
        })
      },
      None,
      Callback::new(
        move |end: TaskEnd<Result<Animation, dovetail_image::Error>>| {
          let result = match end {
            TaskEnd::Done(Ok(a)) => Ok(a),
            TaskEnd::Done(Err(e)) => Err(e.to_string()),
            TaskEnd::Failed(e) => Err(e),
            TaskEnd::Cancelled => Err("cancelled".to_string()),
          };
          let waiting = match &key {
            Some(k) => CACHE.with_borrow_mut(|c| {
              let waiting = match c.remove(k) {
                Some(Entry::Loading(w)) => w,
                _ => Vec::new(),
              };
              if let Ok(a) = &result {
                c.insert(k.clone(), Entry::Ready(a.downgrade(), bytes.clone()));
              }
              waiting
            }),
            None => vec![done.clone()],
          };
          for w in waiting {
            w.call(result.clone());
          }
        },
      ),
    );
  }
}

/// How many PNG, JPEG and GIF files and bytes `Image`s have decoded on
/// this thread. Those that show the same file share one decoding, so this
/// counts each once. For tests.
pub fn image_decodes() -> u64 {
  cache::decodes()
}

/// What an `Image` loads, and its animation's timer, kept between builds.
#[derive(Default)]
struct Loader {
  source: Option<ImageSource>,
  /// The next frame's timer, while an animation plays.
  timer: Option<Task>,
  /// How many times the animation has played through.
  played: u32,
}

/// A picture from a file, bytes, a decoded image or an animation.
///
/// Files and bytes are decoded off the UI thread with a task (M11), once
/// for every `Image` that shows the same file or bytes; until then a
/// placeholder shows, and the window keeps drawing. Without a `width` and
/// `height` the image is its own size, and the placeholder a square of the
/// theme's size for one.
///
/// An animated GIF or APNG plays at its frames' delays, as many times as
/// it says, while `playing` is true; set it to false to stop on the frame
/// shown, as a pause button would.
///
/// With less motion set on the system ([`reduce_motion`]), an animation
/// shows its first frame and does not play (M18). Set `always_play` for
/// one that is the point of the screen, such as a preview of the file.
///
/// [`reduce_motion`]: dovetail::reduce_motion
///
/// `label` names the picture for assistive tools; an empty label marks it
/// as decoration. `density` is image pixels for each logical pixel: 2 for
/// an image drawn for 200% displays. `on_state` is told when it is ready or
/// has failed.
#[component]
pub fn Image(
  #[prop(into)] source: ImageSource,
  #[prop(into)] label: String,
  #[prop(default)] width: Size,
  #[prop(default)] height: Size,
  #[prop(default)] fit: Fit,
  #[prop(default = 1.0)] density: f32,
  #[prop(default = true)] playing: bool,
  #[prop(default)] always_play: bool,
  #[prop(default = Callback::new(|_: ImageState| {}))] on_state: Callback<
    ImageState,
  >,
) -> View {
  let state = use_state(|| ImageState::Loading);
  let animation = use_state(|| None::<Animation>);
  let frame = use_state(|| 0usize);
  let loader = use_state(|| Rc::new(RefCell::new(Loader::default()))).get();
  let changed = loader.borrow().source.as_ref() != Some(&source);
  if changed {
    {
      let mut l = loader.borrow_mut();
      if let Some(t) = l.timer.take() {
        t.cancel();
      }
      l.source = Some(source.clone());
      l.played = 0;
    }
    if frame.try_map(|f| *f != 0).unwrap_or(true) {
      frame.set(0);
    }
    let report = on_state.clone();
    let wanted = source.clone();
    let current = loader.clone();
    let finish = move |result: Result<Animation, String>| {
      // A result for a source no longer shown is dropped.
      if current.borrow().source.as_ref() != Some(&wanted) {
        return;
      }
      let s = match &result {
        Ok(a) => ImageState::Ready(a.first().clone()),
        Err(e) => ImageState::Failed(e.clone()),
      };
      animation.set(result.ok());
      report.call(s.clone());
      state.set(s);
    };
    match source.clone() {
      ImageSource::Bitmap(b) => finish(Ok(Animation::still(b))),
      ImageSource::Animation(a) => finish(Ok(a)),
      ImageSource::File(_) | ImageSource::Bytes(_) => {
        if state
          .try_map(|s| s != &ImageState::Loading)
          .unwrap_or(true)
        {
          state.set(ImageState::Loading);
        }
        if animation.try_map(Option::is_some).unwrap_or(true) {
          animation.set(None);
        }
        cache::load(&source, Callback::new(finish));
      },
    }
  }

  // Less motion stills animations, on their first frame, unless the
  // application asks for this one to play anyway.
  let stilled = !always_play && dovetail::reduce_motion();
  let playing = playing && !stilled;

  // Play an animation: a timer for the frame shown.
  let shown = animation.get();
  let mut at = frame.get();
  if stilled && at != 0 {
    // It starts again from the beginning if motion comes back.
    at = 0;
    frame.set(0);
    loader.borrow_mut().played = 0;
  }
  match &shown {
    Some(a) if a.is_animated() && playing => {
      let mut l = loader.borrow_mut();
      let finished = a.plays().is_some_and(|n| l.played >= n);
      if l.timer.is_none() && !finished {
        let count = a.frames().len();
        let delay = a.frames()[at.min(count - 1)].delay;
        let plays = a.plays();
        let timer_loader = loader.clone();
        l.timer = Some(Task::after(
          delay,
          Callback::new(move |()| {
            let mut l = timer_loader.borrow_mut();
            l.timer = None;
            let mut next = at + 1;
            if next >= count {
              l.played += 1;
              // The last play stops on the last frame.
              next = if plays.is_some_and(|n| l.played >= n) {
                count - 1
              } else {
                0
              };
            }
            drop(l);
            // The image may have gone while the timer waited (B0009).
            let _ = frame.try_update(|f| *f = next);
          }),
        ));
      }
    },
    _ => {
      if let Some(t) = loader.borrow_mut().timer.take() {
        t.cancel();
      }
    },
  }

  let t = theme();
  let frame_box = |children: Vec<View>, own: Option<(f32, f32)>| -> Element {
    let (w, h) = match own {
      Some((w, h)) => (Size::Fixed(w), Size::Fixed(h)),
      None => (Size::Fit, Size::Fit),
    };
    Element::new(ElementKind::Stack)
      .style(Style {
        width: if width == Size::Fit { w } else { width },
        height: if height == Size::Fit { h } else { height },
        align: Align::Stretch,
        clip: true,
        ..Style::default()
      })
      .children(children)
  };
  match (state.get(), shown) {
    (ImageState::Ready(_), Some(a)) => {
      let bitmap = &a.frames()[at.min(a.frames().len() - 1)].bitmap;
      let picture = bitmap.picture_at(density).fit(fit).label(label);
      frame_box(vec![picture.into()], None).into()
    },
    (other, _) => {
      // A placeholder the size the image will be, or a square.
      let side = t.spacing.of(16.0);
      let (mark, description) = match &other {
        ImageState::Failed(why) => (icons::error(), Some(why.clone())),
        _ => (icons::image(), None),
      };
      let mut placeholder = Element::new(ElementKind::Stack)
        .style(Style {
          background: t.colors.surface_container_high,
          text_color: Some(t.colors.on_surface_variant),
          align: Align::Center,
          justify: dovetail::Justify::Center,
          radius: 4.0,
          ..Style::default()
        })
        .children([mark.picture().size((side / 3.0).round()).into()]);
      if !label.is_empty() {
        placeholder = placeholder.role(Role::Image).label(label);
      }
      if let Some(d) = description {
        placeholder = placeholder.description(d);
      }
      frame_box(vec![placeholder.into()], Some((side, side))).into()
    },
  }
}
