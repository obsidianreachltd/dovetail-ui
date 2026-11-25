//! Animated images (D0024): every frame of a GIF or APNG file, made whole
//! when the file is decoded, each with its delay.

use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::{Bitmap, Error};

/// One frame: a whole image, and how long it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
  /// The image.
  pub bitmap: Bitmap,
  /// How long it shows before the next frame.
  pub delay: Duration,
}

#[derive(Debug)]
struct Inner {
  frames: Vec<Frame>,
  loops: Option<u32>,
}

/// An image's frames: one for a still image, several for an animation.
/// Cheap to clone and safe to send between threads, so it can be decoded
/// in a task.
#[derive(Clone, Debug)]
pub struct Animation(Arc<Inner>);

impl PartialEq for Animation {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

/// An [`Animation`] that may have been dropped.
#[derive(Clone, Debug)]
pub struct WeakAnimation(Weak<Inner>);

impl WeakAnimation {
  /// The animation, if it is still kept somewhere.
  pub fn upgrade(&self) -> Option<Animation> {
    self.0.upgrade().map(Animation)
  }
}

impl Animation {
  /// Decodes a PNG, APNG, JPEG or GIF file's bytes: every frame of an
  /// animation, or the one image of a still file.
  pub fn decode(data: &[u8]) -> Result<Animation, Error> {
    let (frames, loops) = if crate::gif::is_gif(data) {
      crate::gif::decode(data)?
    } else if crate::png::is_png(data) {
      let read = crate::png::read(data)?;
      match read.animation {
        Some(animation) => animation,
        None => (vec![(read.image, Duration::ZERO)], Some(1)),
      }
    } else {
      return Ok(Animation::still(Bitmap::decode(data)?));
    };
    Ok(Animation(Arc::new(Inner {
      frames: frames
        .into_iter()
        .map(|(rgba, delay)| Frame { bitmap: Bitmap::from_rgba(rgba), delay })
        .collect(),
      loops,
    })))
  }

  /// Reads and decodes a file, as [`Animation::decode`] does.
  pub fn open(path: impl AsRef<Path>) -> Result<Animation, Error> {
    let data = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    Animation::decode(&data)
  }

  /// A still image, as an animation of one frame.
  pub fn still(bitmap: Bitmap) -> Animation {
    Animation(Arc::new(Inner {
      frames: vec![Frame { bitmap, delay: Duration::ZERO }],
      loops: Some(1),
    }))
  }

  /// The frames, in order. There is always at least one.
  pub fn frames(&self) -> &[Frame] {
    &self.0.frames
  }

  /// The first frame's image: what a still view of it shows.
  pub fn first(&self) -> &Bitmap {
    &self.0.frames[0].bitmap
  }

  /// True when it has more than one frame.
  pub fn is_animated(&self) -> bool {
    self.0.frames.len() > 1
  }

  /// How many times it plays: `None` for ever.
  pub fn plays(&self) -> Option<u32> {
    self.0.loops
  }

  /// A handle that does not keep it alive, for caches.
  pub fn downgrade(&self) -> WeakAnimation {
    WeakAnimation(Arc::downgrade(&self.0))
  }
}
