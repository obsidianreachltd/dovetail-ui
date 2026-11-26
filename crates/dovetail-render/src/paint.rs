//! Painting laid-out boxes with wgpu.
//!
//! - Rectangles, borders and rounded corners come from one instanced
//!   pipeline that works out a signed distance in the fragment shader.
//! - Text is drawn from a glyph atlas made from the built-in font (D0012).
//! - Icons are drawn at the size shown into the same atlas, as glyphs are;
//!   images each have a texture with mipmaps (M17, D0023).
//! - Clipping uses scissor rectangles.
//! - Each window paints into a [`Canvas`], a texture that keeps its pixels
//!   between frames. A frame repaints only its damaged region, then copies
//!   the canvas to the window.

use std::collections::HashMap;
use std::rc::Rc;

use dovetail_core::{Color, Fit, fit_rect};
use dovetail_image::{Bitmap, Icon, WeakBitmap};
use dovetail_layout::{Content, LayoutBox, Rect};
use dovetail_text::Fonts;
use wgpu::util::DeviceExt;

use crate::color::srgb_to_linear;
use crate::font;
use crate::{FrameOutcome, Gpu, WindowSurface};

/// The canvas pixel format: sRGB, so blending happens on linear values.
pub const CANVAS_FORMAT: wgpu::TextureFormat =
  wgpu::TextureFormat::Rgba8UnormSrgb;
const ATLAS_SIZE: u32 = 1024;
/// Icons up to this many pixels across and down go in the glyph atlas;
/// larger ones become images.
const ATLAS_ICON: u32 = 256;
/// Image bytes uploaded in one paint, at most: larger images take several
/// frames, so the window keeps drawing (M17).
const UPLOAD_PER_PAINT: usize = 16 << 20;
/// Image bytes kept on the GPU before the least recently drawn are released.
pub const IMAGE_BUDGET: usize = 256 << 20;
/// Paints after which an image not drawn in any is released.
const IMAGE_IDLE_PAINTS: u64 = 2000;
/// The colour of the keyboard focus ring without a theme.
pub const FOCUS_RING: Color = Color::hex(0x33415c);
/// The width of the focus ring, in logical pixels.
const FOCUS_RING_WIDTH: f32 = 2.0;
/// The colour behind selected text in an element with focus, without a
/// theme.
pub const SELECTION: Color = Color::hex(0xc5cee0);
/// The colour behind selected text in an element without focus.
pub const SELECTION_IDLE: Color = Color::hex(0xdce0e8);

/// A texture that keeps a window's pixels between frames.
pub struct Canvas {
  texture: wgpu::Texture,
  view: wgpu::TextureView,
  bind_group: wgpu::BindGroup,
  width: u32,
  height: u32,
  /// Nothing has been painted yet, so the next paint covers everything.
  fresh: bool,
  generation: u64,
}

impl Canvas {
  /// Width in physical pixels.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// Height in physical pixels.
  pub fn height(&self) -> u32 {
    self.height
  }

  /// True when this canvas was made for `gpu`'s current device.
  pub fn is_current(
    &self,
    gpu: &Gpu,
  ) -> bool {
    self.generation == gpu.generation()
  }
}

/// What [`Painter::paint`] draws.
#[derive(Clone, Copy, Debug)]
pub struct PaintInput<'a> {
  /// The laid-out boxes, in painting order.
  pub boxes: &'a [LayoutBox],
  /// The area to repaint, in logical pixels. `None`, or a new canvas,
  /// repaints everything.
  pub region: Option<Rect>,
  /// The display scale: physical pixels per logical pixel.
  pub scale: f32,
  /// The colour the region is filled with first.
  pub background: Color,
  /// A focus ring to draw over everything: its outer rectangle and corner
  /// radius, in logical pixels.
  pub focus_ring: Option<(Rect, f32)>,
  /// Carets are drawn. Windows turn this off and on to blink them, and off
  /// while the window is in the background.
  pub caret_visible: bool,
  /// The colours of the focus ring and of selected text, from the theme
  /// (M14). Its background is not used: `background` is.
  pub colors: dovetail_core::WindowStyle,
  /// The scrollbars to draw (M15), each with whether it is wide, as
  /// while the pointer is over it. Each is drawn after the last box inside
  /// its element, so layers go over it.
  pub scrollbars: &'a [(dovetail_layout::Scrollbar, bool)],
}

impl<'a> PaintInput<'a> {
  /// Everything in `boxes`, at `scale`, on white, with no focus ring and
  /// carets shown.
  pub fn new(
    boxes: &'a [LayoutBox],
    scale: f32,
  ) -> Self {
    Self {
      boxes,
      region: None,
      scale,
      background: Color::WHITE,
      focus_ring: None,
      caret_visible: true,
      colors: dovetail_core::WindowStyle::default(),
      scrollbars: &[],
    }
  }
}

/// How much one paint did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintStats {
  /// The region painted, in physical pixels: x, y, width, height.
  pub region: [u32; 4],
  /// Rectangles drawn, including the one that clears the region.
  pub rectangles: usize,
  /// Glyphs drawn, icons included.
  pub glyphs: usize,
  /// Images drawn.
  pub images: usize,
  /// Where images still being uploaded were left out, in logical pixels.
  /// Paint this again in the next frame (M17).
  pub waiting: Option<Rect>,
}

impl PaintStats {
  /// Pixels in the painted region.
  pub fn pixels(&self) -> u64 {
    u64::from(self.region[2]) * u64::from(self.region[3])
  }
}

#[derive(Clone, Copy)]
struct AtlasGlyph {
  x: u32,
  y: u32,
  width: u32,
  height: u32,
  /// Pixels from the pen to the bitmap's left edge.
  left: i32,
  /// Pixels from the baseline up to the bitmap's top edge.
  top: i32,
}

/// Which glyph image an atlas entry holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum GlyphKey {
  /// A real font's glyph: font number, glyph, pixel size, and weight
  /// (each weight has its own fonts, M14).
  Font(usize, u16, u32, u16),
  /// The built-in stroke font (D0012), used when no font can be read.
  Stroke(char, u32),
  /// An icon drawn at a width and height in pixels (M17).
  Icon(u64, u32, u32),
}

/// A glyph drawn and ready to upload.
struct GlyphImage {
  width: u32,
  height: u32,
  left: i32,
  top: i32,
  coverage: Vec<u8>,
}

/// Glyphs drawn once and kept in a texture, packed in rows.
struct Atlas {
  texture: wgpu::Texture,
  glyphs: HashMap<GlyphKey, AtlasGlyph>,
  cursor: (u32, u32),
  row_height: u32,
}

impl Atlas {
  fn new(device: &wgpu::Device) -> Self {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
      label: Some("dovetail glyph atlas"),
      size: wgpu::Extent3d {
        width: ATLAS_SIZE,
        height: ATLAS_SIZE,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::R8Unorm,
      usage: wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_DST,
      view_formats: &[],
    });
    Self {
      texture,
      glyphs: HashMap::new(),
      cursor: (0, 0),
      row_height: 0,
    }
  }

  /// The glyph for `key`, drawn with `draw` and put into the atlas the first
  /// time. When the atlas is full it starts again, and `None` asks the
  /// caller to repaint everything.
  fn glyph(
    &mut self,
    queue: &wgpu::Queue,
    key: GlyphKey,
    draw: impl FnOnce() -> GlyphImage,
  ) -> Option<AtlasGlyph> {
    if let Some(glyph) = self.glyphs.get(&key) {
      return Some(*glyph);
    }
    let bitmap = draw();
    if bitmap.width == 0 || bitmap.height == 0 {
      let empty = AtlasGlyph {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        left: 0,
        top: 0,
      };
      self.glyphs.insert(key, empty);
      return Some(empty);
    }
    if self.cursor.0 + bitmap.width > ATLAS_SIZE {
      self.cursor = (0, self.cursor.1 + self.row_height + 1);
      self.row_height = 0;
    }
    if self.cursor.1 + bitmap.height > ATLAS_SIZE || bitmap.width > ATLAS_SIZE {
      self.glyphs.clear();
      self.cursor = (0, 0);
      self.row_height = 0;
      return None;
    }
    let glyph = AtlasGlyph {
      x: self.cursor.0,
      y: self.cursor.1,
      width: bitmap.width,
      height: bitmap.height,
      left: bitmap.left,
      top: bitmap.top,
    };
    queue.write_texture(
      wgpu::TexelCopyTextureInfo {
        texture: &self.texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x: glyph.x, y: glyph.y, z: 0 },
        aspect: wgpu::TextureAspect::All,
      },
      &bitmap.coverage,
      wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(bitmap.width),
        rows_per_image: None,
      },
      wgpu::Extent3d {
        width: bitmap.width,
        height: bitmap.height,
        depth_or_array_layers: 1,
      },
    );
    self.cursor.0 += bitmap.width + 1;
    self.row_height = self.row_height.max(bitmap.height);
    self.glyphs.insert(key, glyph);
    Some(glyph)
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Pipeline {
  Rect,
  Glyph,
  /// An image: its key in the painter's images.
  Image(u64),
  /// A canvas's own drawing: an index into the canvases to draw.
  Canvas(usize),
}

/// A canvas to draw in this paint: its painter, its viewport and scissor.
type CanvasJob = (Rc<dyn CanvasPainter>, [f32; 4], [u32; 4]);

/// What an application's canvas is given to draw with (M09).
///
/// The canvas records its own render pass into `encoder`, drawing into
/// `target`, with the existing pixels kept (load, not clear). It should
/// draw inside `viewport` and only inside `scissor`: [`CanvasFrame::begin`]
/// starts a pass set up that way. It is called again whenever the region
/// it is in is painted, and must draw everything there each time.
pub struct CanvasFrame<'a> {
  /// The GPU device, to make buffers and pipelines.
  pub device: &'a wgpu::Device,
  /// The queue, to write buffers.
  pub queue: &'a wgpu::Queue,
  /// The frame's command encoder; the canvas's pass goes here, in order
  /// with the rest of the frame.
  pub encoder: &'a mut wgpu::CommandEncoder,
  /// The texture being painted.
  pub target: &'a wgpu::TextureView,
  /// Its format: pipelines must draw to it.
  pub format: wgpu::TextureFormat,
  /// The canvas's box in physical pixels: x, y, width and height. It may
  /// reach outside the texture.
  pub viewport: [f32; 4],
  /// The pixels that may be drawn now: x, y, width and height, inside the
  /// canvas, its clipping parents and the region being painted.
  pub scissor: [u32; 4],
  /// Physical pixels per logical pixel.
  pub scale: f32,
  /// The GPU device's generation: when it changes, the device was lost
  /// and replaced, and pipelines and buffers must be made again.
  pub generation: u64,
}

impl CanvasFrame<'_> {
  /// Starts a render pass that keeps what is drawn, with the viewport and
  /// scissor set, so normalised device coordinates cover the canvas.
  pub fn begin(
    &mut self,
    label: &str,
  ) -> wgpu::RenderPass<'_> {
    let mut pass =
      self
        .encoder
        .begin_render_pass(&wgpu::RenderPassDescriptor {
          label: Some(label),
          color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: self.target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
              load: wgpu::LoadOp::Load,
              store: wgpu::StoreOp::Store,
            },
          })],
          ..Default::default()
        });
    let [x, y, w, h] = self.viewport;
    pass.set_viewport(x, y, w, h, 0.0, 1.0);
    let [sx, sy, sw, sh] = self.scissor;
    pass.set_scissor_rect(sx, sy, sw, sh);
    pass
  }
}

/// Draws a canvas element's content with wgpu (M09). Make the element's
/// source with [`canvas`].
pub trait CanvasPainter {
  /// Records the canvas's drawing into `frame`.
  fn paint(
    &self,
    frame: &mut CanvasFrame<'_>,
  );
}

/// A canvas element's source, drawn by `painter`. Give it to
/// `Element::canvas`. A new source repaints the canvas, so make one when
/// what it shows changes.
pub fn canvas(
  painter: impl CanvasPainter + 'static
) -> dovetail_core::CanvasSource {
  dovetail_core::CanvasSource::new(Rc::new(painter) as Rc<dyn CanvasPainter>)
}

struct Batch {
  pipeline: Pipeline,
  scissor: [u32; 4],
  start: u32,
  end: u32,
}

/// An image's texture (M17), uploaded a part at a time.
struct GpuImage {
  texture: wgpu::Texture,
  bind_group: wgpu::BindGroup,
  bytes: usize,
  /// The paint it was last drawn in.
  last_drawn: u64,
  /// Tells when the image is gone, so the texture can go too. Icons drawn
  /// as images have none, and go when not drawn for a while.
  weak: Option<WeakBitmap>,
  /// The image, kept while it is still being uploaded.
  uploading: Option<Bitmap>,
  /// The first of the image's levels in the texture: later ones when the
  /// first are larger than the GPU allows.
  first_level: usize,
  /// The next level and row to upload.
  next: (usize, u32),
}

/// Paints laid-out boxes into canvases, and copies canvases to windows.
///
/// A painter belongs to one GPU device. After a device loss, make a new
/// painter and new canvases (D0009).
pub struct Painter {
  rect_pipeline: wgpu::RenderPipeline,
  glyph_pipeline: wgpu::RenderPipeline,
  blit_layout: wgpu::PipelineLayout,
  blit_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
  shader: wgpu::ShaderModule,
  globals: wgpu::Buffer,
  globals_group: wgpu::BindGroup,
  canvas_layout: wgpu::BindGroupLayout,
  atlas: Atlas,
  atlas_group: wgpu::BindGroup,
  /// The fonts text is drawn with.
  fonts: Fonts,
  sampler: wgpu::Sampler,
  generation: u64,
  image_pipeline: wgpu::RenderPipeline,
  image_layout: wgpu::BindGroupLayout,
  /// Smooth, with mipmaps, for images.
  image_sampler: wgpu::Sampler,
  images: HashMap<u64, GpuImage>,
  image_budget: usize,
  /// How many paints there have been, to tell how recently an image was
  /// drawn.
  paints: u64,
}

impl Painter {
  /// Makes the pipelines and the glyph atlas on `gpu`'s device.
  pub fn new(gpu: &Gpu) -> Self {
    let device = gpu.device();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("dovetail shaders"),
      source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
    });

    let globals_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("dovetail globals"),
        entries: &[wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::VERTEX,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        }],
      });
    let texture_layout = |label| {
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
          wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
              sample_type: wgpu::TextureSampleType::Float { filterable: true },
              view_dimension: wgpu::TextureViewDimension::D2,
              multisampled: false,
            },
            count: None,
          },
          wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
          },
        ],
      })
    };
    let atlas_layout = texture_layout("dovetail atlas");
    let canvas_layout = texture_layout("dovetail canvas");
    let image_layout = texture_layout("dovetail image");
    let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("dovetail smooth"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      mipmap_filter: wgpu::MipmapFilterMode::Linear,
      ..Default::default()
    });

    let globals = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("dovetail globals"),
      size: 16,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("dovetail globals"),
      layout: &globals_layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: globals.as_entire_binding(),
      }],
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("dovetail nearest"),
      ..Default::default()
    });
    let atlas = Atlas::new(device);
    let atlas_view = atlas
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let atlas_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("dovetail atlas"),
      layout: &atlas_layout,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&atlas_view),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Sampler(&sampler),
        },
      ],
    });

    let paint_layout =
      device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("dovetail paint"),
        bind_group_layouts: &[
          Some(&globals_layout),
          Some(&atlas_layout),
        ],
        immediate_size: 0,
      });
    let blit_layout =
      device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("dovetail blit"),
        bind_group_layouts: &[None, None, Some(&canvas_layout)],
        immediate_size: 0,
      });

    let f32x4 = |location| wgpu::VertexAttribute {
      format: wgpu::VertexFormat::Float32x4,
      offset: u64::from(location) * 16,
      shader_location: location,
    };
    let rect_attributes = [f32x4(0), f32x4(1), f32x4(2), f32x4(3)];
    let glyph_attributes = [f32x4(0), f32x4(1), f32x4(2), f32x4(3)];
    let image_attributes = [f32x4(0), f32x4(1), f32x4(2)];
    let instance_pipeline =
      |label, vs, fs, attributes: &[wgpu::VertexAttribute], blend| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
          label: Some(label),
          layout: Some(&paint_layout),
          vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(vs),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
              array_stride: attributes.len() as u64 * 16,
              step_mode: wgpu::VertexStepMode::Instance,
              attributes,
            })],
          },
          primitive: wgpu::PrimitiveState::default(),
          depth_stencil: None,
          multisample: wgpu::MultisampleState::default(),
          fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
              format: CANVAS_FORMAT,
              blend: Some(blend),
              write_mask: wgpu::ColorWrites::ALL,
            })],
          }),
          multiview_mask: None,
          cache: None,
        })
      };
    let straight = wgpu::BlendState::ALPHA_BLENDING;
    let rect_pipeline = instance_pipeline(
      "dovetail rectangles",
      "rect_vs",
      "rect_fs",
      &rect_attributes,
      straight,
    );
    let glyph_pipeline = instance_pipeline(
      "dovetail glyphs",
      "glyph_vs",
      "glyph_fs",
      &glyph_attributes,
      straight,
    );
    let image_pipeline = instance_pipeline(
      "dovetail images",
      "image_vs",
      "image_fs",
      &image_attributes,
      wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
    );

    Self {
      rect_pipeline,
      glyph_pipeline,
      blit_layout,
      blit_pipelines: HashMap::new(),
      shader,
      globals,
      globals_group,
      canvas_layout,
      atlas,
      atlas_group,
      fonts: Fonts::system(),
      sampler,
      generation: gpu.generation(),
      image_pipeline,
      image_layout,
      image_sampler,
      images: HashMap::new(),
      image_budget: IMAGE_BUDGET,
      paints: 0,
    }
  }

  /// Sets how many bytes of images are kept on the GPU before the least
  /// recently drawn are released. The default is [`IMAGE_BUDGET`].
  pub fn set_image_budget(
    &mut self,
    bytes: usize,
  ) {
    self.image_budget = bytes;
  }

  /// How many images are on the GPU, and the bytes they take.
  pub fn image_memory(&self) -> (usize, usize) {
    (
      self.images.len(),
      self.images.values().map(|i| i.bytes).sum(),
    )
  }

  /// Makes the texture for image `key`, or finds it, and uploads more of it
  /// within `budget` bytes. Returns true once it is all uploaded.
  fn image_ready(
    &mut self,
    gpu: &Gpu,
    key: u64,
    bitmap: impl FnOnce() -> Bitmap,
    weak: bool,
    budget: &mut usize,
  ) -> bool {
    let paint = self.paints;
    if !self.images.contains_key(&key) {
      let bitmap = bitmap();
      let device = gpu.device();
      let limit = device.limits().max_texture_dimension_2d;
      let levels = bitmap.levels();
      let first_level = levels
        .iter()
        .position(|l| l.width <= limit && l.height <= limit)
        .unwrap_or(levels.len() - 1);
      let top = &levels[first_level];
      let count = (levels.len() - first_level) as u32;
      let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dovetail image"),
        size: wgpu::Extent3d {
          width: top.width,
          height: top.height,
          depth_or_array_layers: 1,
        },
        mip_level_count: count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
          | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
      });
      let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
      let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("dovetail image"),
        layout: &self.image_layout,
        entries: &[
          wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
          },
          wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&self.image_sampler),
          },
        ],
      });
      let bytes = levels[first_level..]
        .iter()
        .map(|l| l.pixels.len())
        .sum();
      self.images.insert(
        key,
        GpuImage {
          texture,
          bind_group,
          bytes,
          last_drawn: paint,
          weak: weak.then(|| bitmap.downgrade()),
          uploading: Some(bitmap),
          first_level,
          next: (first_level, 0),
        },
      );
    }
    let image = self.images.get_mut(&key).expect("just made");
    image.last_drawn = paint;
    let Some(bitmap) = image.uploading.clone() else {
      return true;
    };
    let levels = bitmap.levels();
    while *budget > 0 {
      let (level, row) = image.next;
      let Some(l) = levels.get(level) else {
        break;
      };
      let row_bytes = l.width as usize * 4;
      let rows = ((*budget / row_bytes).max(1) as u32).min(l.height - row);
      let start = row as usize * row_bytes;
      let end = start + rows as usize * row_bytes;
      gpu.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
          texture: &image.texture,
          mip_level: (level - image.first_level) as u32,
          origin: wgpu::Origin3d { x: 0, y: row, z: 0 },
          aspect: wgpu::TextureAspect::All,
        },
        &l.pixels[start..end],
        wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(row_bytes as u32),
          rows_per_image: None,
        },
        wgpu::Extent3d {
          width: l.width,
          height: rows,
          depth_or_array_layers: 1,
        },
      );
      *budget = budget.saturating_sub(end - start);
      image.next = if row + rows >= l.height {
        (level + 1, 0)
      } else {
        (level, row + rows)
      };
    }
    if image.next.0 >= levels.len() {
      image.uploading = None;
      true
    } else {
      false
    }
  }

  /// Releases images that are gone, not drawn for a long time, or past the
  /// budget, least recently drawn first. Images drawn in this paint stay.
  fn release_images(&mut self) {
    let paint = self.paints;
    self.images.retain(|_, image| {
      image.last_drawn == paint
        || (image
          .weak
          .as_ref()
          .is_none_or(WeakBitmap::is_alive)
          && paint - image.last_drawn < IMAGE_IDLE_PAINTS)
    });
    let mut total: usize = self.images.values().map(|i| i.bytes).sum();
    if total <= self.image_budget {
      return;
    }
    let mut order: Vec<(u64, u64, usize)> = self
      .images
      .iter()
      .map(|(k, i)| (i.last_drawn, *k, i.bytes))
      .collect();
    order.sort_unstable();
    for (last, key, bytes) in order {
      if total <= self.image_budget || last == paint {
        break;
      }
      self.images.remove(&key);
      total -= bytes;
    }
  }

  /// True when this painter was made for `gpu`'s current device.
  pub fn is_current(
    &self,
    gpu: &Gpu,
  ) -> bool {
    self.generation == gpu.generation()
  }

  /// A canvas of `width` × `height` physical pixels.
  pub fn canvas(
    &self,
    gpu: &Gpu,
    width: u32,
    height: u32,
  ) -> Canvas {
    let (width, height) = (width.max(1), height.max(1));
    let texture = gpu
      .device()
      .create_texture(&wgpu::TextureDescriptor {
        label: Some("dovetail canvas"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: CANVAS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
          | wgpu::TextureUsages::TEXTURE_BINDING
          | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
      });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group =
      gpu
        .device()
        .create_bind_group(&wgpu::BindGroupDescriptor {
          label: Some("dovetail canvas"),
          layout: &self.canvas_layout,
          entries: &[
            wgpu::BindGroupEntry {
              binding: 0,
              resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
              binding: 1,
              resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
          ],
        });
    Canvas {
      texture,
      view,
      bind_group,
      width,
      height,
      fresh: true,
      generation: gpu.generation(),
    }
  }

  /// Draws text with the built-in font instead of the system's fonts
  /// from now on, so what is painted is the same on every system, as
  /// tests that compare pictures need. Its letters measure as
  /// `dovetail_layout::FixedPitch` does.
  pub fn use_built_in_font(&mut self) {
    self.fonts = dovetail_text::Fonts::from_files([]);
  }

  /// Paints `input` into `canvas`. See [`PaintInput`].
  pub fn paint(
    &mut self,
    gpu: &Gpu,
    canvas: &mut Canvas,
    input: &PaintInput<'_>,
  ) -> PaintStats {
    let PaintInput {
      boxes,
      region,
      scale,
      background,
      focus_ring,
      caret_visible,
      colors,
      scrollbars,
    } = *input;
    // Bars in the order they are drawn: after the last box inside their
    // element, inner bars first.
    let mut bars: Vec<&(dovetail_layout::Scrollbar, bool)> =
      scrollbars.iter().collect();
    bars.sort_by_key(|(b, _)| b.after);
    let mut bars = bars.into_iter().peekable();
    debug_assert_eq!(
      canvas.generation,
      gpu.generation(),
      "canvas from a lost device"
    );
    let full = [0, 0, canvas.width, canvas.height];
    let region = match region {
      Some(r) if !canvas.fresh => intersect(to_pixels(r, scale), full),
      _ => full,
    };
    canvas.fresh = false;
    if region[2] == 0 || region[3] == 0 {
      return PaintStats { region, ..PaintStats::default() };
    }
    self.paints += 1;
    let mut upload_budget = UPLOAD_PER_PAINT;
    let mut waiting: Option<Rect> = None;

    let mut rects: Vec<f32> = Vec::new();
    let mut glyphs: Vec<f32> = Vec::new();
    let mut images: Vec<f32> = Vec::new();
    let mut batches: Vec<Batch> = Vec::new();
    let push = |batches: &mut Vec<Batch>,
                pipeline: Pipeline,
                scissor: [u32; 4],
                index: u32| match batches.last_mut() {
      Some(b)
        if b.pipeline == pipeline && b.scissor == scissor && b.end == index =>
      {
        b.end += 1
      },
      _ => batches.push(Batch {
        pipeline,
        scissor,
        start: index,
        end: index + 1,
      }),
    };

    // Clear the region.
    let r = [
      region[0] as f32,
      region[1] as f32,
      region[2] as f32,
      region[3] as f32,
    ];
    rects.extend(r);
    rects.extend(linear(background.with_alpha(1.0)));
    rects.extend([0.0; 8]);
    push(&mut batches, Pipeline::Rect, region, 0);

    let mut atlas_full = false;
    // Canvases to draw, with their viewports and scissors, in order.
    let mut canvases: Vec<CanvasJob> = Vec::new();
    // Carets go over everything else, so they are drawn last.
    let mut carets: Vec<(Rect, Color, [u32; 4])> = Vec::new();
    let measure = crate::TextMeasure { fonts: self.fonts.clone() };
    // What each box is drawn over, in linear light: the background of the
    // nearest element below it, over whatever that is drawn over. Text uses
    // it to keep its weight (see `glyph_fs`). Parents come before their
    // children in `boxes`.
    let mut backdrop = vec![linear(background.with_alpha(1.0)); boxes.len()];
    for (i, b) in boxes.iter().enumerate() {
      let under = backdrop[i];
      let own = match &b.content {
        Content::Element(e) if e.style.background.a * b.opacity > 0.0 => {
          let c = linear(e.style.background);
          let a = e.style.background.a * b.opacity;
          [
            under[0] + (c[0] - under[0]) * a,
            under[1] + (c[1] - under[1]) * a,
            under[2] + (c[2] - under[2]) * a,
            1.0,
          ]
        },
        _ => under,
      };
      for &c in &b.children {
        if let Some(slot) = backdrop.get_mut(c) {
          *slot = own;
        }
      }
    }
    for (i, b) in boxes.iter().enumerate() {
      // Bars whose content is all drawn.
      while let Some((bar, wide)) = bars.next_if(|(bar, _)| bar.after < i) {
        scrollbar(&mut rects, &mut batches, bar, *wide, colors, scale, region);
      }
      let scissor = intersect(to_pixels(b.clip, scale), region);
      // How solid this box is, with its parents (M18). Everything it
      // draws, apart from a canvas, has its alpha multiplied by it.
      let fade = |c: Color| Color { a: c.a * b.opacity, ..c };
      let mut selection: Vec<(Rect, Color, [u32; 4])> = Vec::new();
      if let Content::Element(e) = &b.content
        && let Some(cursor) = &e.text_cursor
        && let Some(t) = dovetail_layout::first_text(boxes, i)
      {
        let text = &boxes[t];
        let geometry = dovetail_layout::cursor_geometry(text, cursor, &measure);
        let clip = intersect(to_pixels(text.clip, scale), region);
        let tint = if cursor.show_caret {
          colors.selection
        } else {
          colors.selection_idle
        };
        // The selection goes over the element's background and under the
        // text, which comes later.
        selection.extend(
          geometry
            .selection
            .iter()
            .map(|r| (*r, fade(tint), clip)),
        );
        if let Content::Text { color, .. } = &text.content {
          // The caret and the underline under text being composed.
          let caret = geometry.caret.filter(|_| caret_visible);
          for r in caret.iter().chain(&geometry.composing) {
            carets.push((
              *r,
              Color { a: color.a * text.opacity, ..*color },
              clip,
            ));
          }
        }
      }
      if scissor[2] == 0
        || scissor[3] == 0
        || !overlaps(to_pixels(b.rect, scale), scissor)
      {
        continue;
      }
      let rect = [
        b.rect.x * scale,
        b.rect.y * scale,
        b.rect.width * scale,
        b.rect.height * scale,
      ];
      match &b.content {
        Content::Element(e) => {
          let s = &e.style;
          let visible = b.opacity > 0.0
            && (s.background.a > 0.0
              || (s.border.width > 0.0 && s.border.color.a > 0.0));
          if visible {
            let index = (rects.len() / 16) as u32;
            rects.extend(rect);
            rects.extend(linear(fade(s.background)));
            rects.extend(linear(fade(s.border.color)));
            rects.extend([
              s.radius * scale,
              s.border.width * scale,
              0.0,
              0.0,
            ]);
            push(&mut batches, Pipeline::Rect, scissor, index);
          }
          for (r, tint, clip) in selection {
            let index = (rects.len() / 16) as u32;
            rects.extend(snap(r, scale));
            rects.extend(linear(tint));
            rects.extend([0.0; 8]);
            push(&mut batches, Pipeline::Rect, clip, index);
          }
          // A canvas draws itself here, inside its border, above its
          // background and below what comes after it.
          if let Some(painter) = e
            .canvas
            .as_ref()
            .and_then(|c| c.get::<Rc<dyn CanvasPainter>>())
          {
            let inset = s.border.width;
            let inner = Rect::new(
              b.rect.x + inset,
              b.rect.y + inset,
              (b.rect.width - 2.0 * inset).max(0.0),
              (b.rect.height - 2.0 * inset).max(0.0),
            );
            let clip =
              intersect(intersect(to_pixels(inner, scale), scissor), region);
            if clip[2] > 0
              && clip[3] > 0
              && inner.width > 0.0
              && inner.height > 0.0
            {
              canvases.push((
                painter.clone(),
                [
                  inner.x * scale,
                  inner.y * scale,
                  inner.width * scale,
                  inner.height * scale,
                ],
                clip,
              ));
              batches.push(Batch {
                pipeline: Pipeline::Canvas(canvases.len() - 1),
                scissor: clip,
                start: 0,
                end: 0,
              });
            }
          }
        },
        Content::Picture { .. } | Content::Text { .. } if b.opacity <= 0.0 => {
        },
        Content::Picture { picture, color, .. } => {
          // An icon drawn as an image keeps its colour, so it is drawn once;
          // the image's own opacity fades it.
          let solid = *color;
          let color = &fade(*color);
          if let Some(icon) = picture.source::<Icon>() {
            let fit = if picture.fit == Fit::Stretch {
              Fit::Stretch
            } else {
              Fit::Contain
            };
            let (dest, _) = fit_rect(fit, (icon.aspect(), 1.0), rect);
            // On whole pixels, so the icon is drawn pixel for pixel.
            let x = dest[0].round();
            let y = dest[1].round();
            let w = dest[2].round().max(1.0) as u32;
            let h = dest[3].round().max(1.0) as u32;
            if w <= ATLAS_ICON && h <= ATLAS_ICON && !icon.is_multicolour() {
              let key = GlyphKey::Icon(icon.id(), w, h);
              let draw = || {
                let c = icon.render(w, h);
                GlyphImage {
                  width: c.width,
                  height: c.height,
                  left: 0,
                  top: 0,
                  coverage: c.alpha,
                }
              };
              let Some(g) = self.atlas.glyph(gpu.queue(), key, draw) else {
                atlas_full = true;
                continue;
              };
              if g.width > 0 {
                let index = (glyphs.len() / 16) as u32;
                glyphs.extend([x, y, g.width as f32, g.height as f32]);
                let atlas = ATLAS_SIZE as f32;
                glyphs.extend([
                  g.x as f32 / atlas,
                  g.y as f32 / atlas,
                  g.width as f32 / atlas,
                  g.height as f32 / atlas,
                ]);
                glyphs.extend(linear(*color));
                glyphs.extend(backdrop[i]);
                push(&mut batches, Pipeline::Glyph, scissor, index);
              }
            } else {
              // A large icon, or one with colours of its own (D0024), is
              // drawn once at this size, in its colours, as an image.
              let key = large_icon_key(icon.id(), w, h, solid);
              let ink = solid;
              let make = || {
                Bitmap::from_rgba(icon.render_rgba(
                  w,
                  h,
                  [ink.r, ink.g, ink.b, ink.a],
                ))
              };
              if self.image_ready(gpu, key, make, false, &mut upload_budget) {
                let index = (images.len() / 12) as u32;
                images.extend([x, y, w as f32, h as f32]);
                images.extend([0.0, 0.0, 1.0, 1.0]);
                images.extend([b.opacity, 0.0, 0.0, 0.0]);
                push(&mut batches, Pipeline::Image(key), scissor, index);
              } else {
                waiting = Some(waiting.map_or(b.rect, |w| w.union(&b.rect)));
              }
            }
          } else if let Some(bitmap) = picture.source::<Bitmap>() {
            let natural = (bitmap.width() as f32, bitmap.height() as f32);
            let (dest, part) = fit_rect(picture.fit, natural, rect);
            // Edges on whole pixels, so an image drawn at its pixel size is
            // drawn pixel for pixel.
            let x0 = dest[0].round();
            let y0 = dest[1].round();
            let x1 = (dest[0] + dest[2]).round();
            let y1 = (dest[1] + dest[3]).round();
            if x1 > x0 && y1 > y0 {
              let key = bitmap.id();
              let ready = self.image_ready(
                gpu,
                key,
                || bitmap.clone(),
                true,
                &mut upload_budget,
              );
              if ready {
                let index = (images.len() / 12) as u32;
                images.extend([x0, y0, x1 - x0, y1 - y0]);
                images.extend(part);
                images.extend([b.opacity, 0.0, 0.0, 0.0]);
                push(&mut batches, Pipeline::Image(key), scissor, index);
              } else {
                waiting = Some(waiting.map_or(b.rect, |w| w.union(&b.rect)));
              }
            }
          }
        },
        Content::Text { text, font_size, color, weight } => {
          let color = &fade(*color);
          // Each weight has its own fonts (M14).
          let weighted = if *weight == dovetail_layout::FontWeight::REGULAR {
            self.fonts.clone()
          } else {
            dovetail_text::Fonts::system_weight(weight.0)
          };
          let pixel_size = font_size * scale;
          let size_key = pixel_size.to_bits();
          // Each glyph's key, pen position and baseline, in pixels. Pens and
          // baselines sit on whole pixels, so text stays sharp.
          let placed: Vec<(GlyphKey, f32, f32)> = if self.fonts.is_empty() {
            let advance = font::ADVANCE * pixel_size;
            let line_height = font::LINE * pixel_size;
            text
              .split('\n')
              .enumerate()
              .flat_map(|(row, line)| {
                line
                  .chars()
                  .enumerate()
                  .filter(|(_, c)| *c != ' ')
                  .map(move |(i, c)| {
                    (
                      GlyphKey::Stroke(c, size_key),
                      (rect[0] + i as f32 * advance).round(),
                      (rect[1] + row as f32 * line_height).round(),
                    )
                  })
              })
              .collect()
          } else {
            // One line per `\n`, each one line height below the last.
            let line_height = weighted.line_metrics(pixel_size).line_height();
            let mut placed = Vec::new();
            for (i, text) in text.split('\n').enumerate() {
              let line = weighted.shape(text, pixel_size);
              let baseline =
                (rect[1] + i as f32 * line_height + line.metrics.ascent)
                  .round();
              // Right-to-left lines line up on the right of the box (M13).
              let start = if line.rtl {
                (rect[2] - line.width).max(0.0)
              } else {
                0.0
              };
              placed.extend(line.glyphs.iter().map(|g| {
                (
                  GlyphKey::Font(g.font, g.glyph, size_key, weight.0),
                  (rect[0] + start + g.x).round(),
                  // Marks are raised or lowered onto their letter.
                  (baseline - g.y).round(),
                )
              }));
            }
            placed
          };
          for (key, pen, baseline) in placed {
            let fonts = &weighted;
            let draw = || match key {
              GlyphKey::Font(font, glyph, _, _) => {
                let b = fonts.rasterize(font, glyph, pixel_size, 0.0);
                GlyphImage {
                  width: b.width,
                  height: b.height,
                  left: b.left,
                  top: b.top,
                  coverage: b.coverage,
                }
              },
              GlyphKey::Stroke(c, _) => {
                let b = font::rasterize(c, pixel_size);
                GlyphImage {
                  width: b.width,
                  height: b.height,
                  left: 0,
                  top: 0,
                  coverage: b.coverage,
                }
              },
              // Text never asks for an icon.
              GlyphKey::Icon(..) => GlyphImage {
                width: 0,
                height: 0,
                left: 0,
                top: 0,
                coverage: Vec::new(),
              },
            };
            let Some(g) = self.atlas.glyph(gpu.queue(), key, draw) else {
              atlas_full = true;
              break;
            };
            if g.width == 0 {
              continue;
            }
            let index = (glyphs.len() / 16) as u32;
            let (x, y) = (pen + g.left as f32, baseline - g.top as f32);
            glyphs.extend([x, y, g.width as f32, g.height as f32]);
            let atlas = ATLAS_SIZE as f32;
            glyphs.extend([
              g.x as f32 / atlas,
              g.y as f32 / atlas,
              g.width as f32 / atlas,
              g.height as f32 / atlas,
            ]);
            glyphs.extend(linear(*color));
            glyphs.extend(backdrop[i]);
            push(&mut batches, Pipeline::Glyph, scissor, index);
          }
        },
      }
    }
    for (bar, wide) in bars {
      scrollbar(&mut rects, &mut batches, bar, *wide, colors, scale, region);
    }
    for (r, color, clip) in carets {
      // At least one physical pixel wide, on whole pixels.
      let mut px = snap(r, scale);
      px[2] = px[2].max(1.0);
      px[3] = px[3].max(1.0);
      let index = (rects.len() / 16) as u32;
      rects.extend(px);
      rects.extend(linear(color));
      rects.extend([0.0; 8]);
      push(&mut batches, Pipeline::Rect, clip, index);
    }
    // The focus ring goes over everything, inside the repaint region.
    if let Some((r, radius)) = focus_ring {
      let index = (rects.len() / 16) as u32;
      rects.extend([
        r.x * scale,
        r.y * scale,
        r.width * scale,
        r.height * scale,
      ]);
      rects.extend(linear(Color { a: 0.0, ..colors.focus_ring }));
      rects.extend(linear(colors.focus_ring));
      rects.extend([
        radius * scale,
        FOCUS_RING_WIDTH * scale,
        0.0,
        0.0,
      ]);
      push(&mut batches, Pipeline::Rect, region, index);
    }
    if atlas_full {
      // The atlas started again; paint everything with fresh glyphs.
      canvas.fresh = true;
      return self.paint(gpu, canvas, &PaintInput { region: None, ..*input });
    }

    let device = gpu.device();
    gpu.queue().write_buffer(
      &self.globals,
      0,
      &bytes(&[
        canvas.width as f32,
        canvas.height as f32,
        0.0,
        0.0,
      ]),
    );
    let rect_buffer = instance_buffer(device, "dovetail rectangles", &rects);
    let glyph_buffer = (!glyphs.is_empty())
      .then(|| instance_buffer(device, "dovetail glyphs", &glyphs));
    let image_buffer = (!images.is_empty())
      .then(|| instance_buffer(device, "dovetail images", &images));

    let mut encoder =
      device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("dovetail paint"),
      });
    // One render pass for each run of batches, with each canvas's own pass
    // recorded between them, so the frame keeps its order.
    let mut next = 0;
    while next < batches.len() {
      if let Pipeline::Canvas(i) = batches[next].pipeline {
        let (painter, viewport, scissor) = &canvases[i];
        let mut frame = CanvasFrame {
          device,
          queue: gpu.queue(),
          encoder: &mut encoder,
          target: &canvas.view,
          format: CANVAS_FORMAT,
          viewport: *viewport,
          scissor: *scissor,
          scale,
          generation: gpu.generation(),
        };
        painter.paint(&mut frame);
        next += 1;
        continue;
      }
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("dovetail paint"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &canvas.view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        ..Default::default()
      });
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_bind_group(1, &self.atlas_group, &[]);
      while let Some(batch) = batches.get(next) {
        if matches!(batch.pipeline, Pipeline::Canvas(_)) {
          break;
        }
        let [x, y, w, h] = batch.scissor;
        pass.set_scissor_rect(x, y, w, h);
        match batch.pipeline {
          Pipeline::Rect => {
            pass.set_pipeline(&self.rect_pipeline);
            pass.set_vertex_buffer(0, rect_buffer.slice(..));
          },
          Pipeline::Glyph => {
            pass.set_pipeline(&self.glyph_pipeline);
            pass.set_bind_group(1, &self.atlas_group, &[]);
            pass.set_vertex_buffer(
              0,
              glyph_buffer.as_ref().expect("glyphs").slice(..),
            );
          },
          Pipeline::Image(key) => {
            pass.set_pipeline(&self.image_pipeline);
            pass.set_bind_group(1, &self.images[&key].bind_group, &[]);
            pass.set_vertex_buffer(
              0,
              image_buffer.as_ref().expect("images").slice(..),
            );
          },
          Pipeline::Canvas(_) => unreachable!("handled above"),
        }
        pass.draw(0..6, batch.start..batch.end);
        next += 1;
      }
    }
    gpu.queue().submit([encoder.finish()]);
    self.release_images();
    PaintStats {
      region,
      rectangles: rects.len() / 16,
      glyphs: glyphs.len() / 16,
      images: images.len() / 12,
      waiting,
    }
  }

  /// Copies `canvas` to the window and shows it.
  pub fn present(
    &mut self,
    gpu: &Gpu,
    canvas: &Canvas,
    surface: &mut WindowSurface,
  ) -> FrameOutcome {
    let Some(format) = surface.format() else {
      return FrameOutcome::Skipped;
    };
    if !self.blit_pipelines.contains_key(&format) {
      let pipeline = self.blit_pipeline(gpu, format);
      self.blit_pipelines.insert(format, pipeline);
    }
    let pipeline = &self.blit_pipelines[&format];
    surface.draw_with(gpu, |encoder, view| {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("dovetail present"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
          },
        })],
        ..Default::default()
      });
      pass.set_pipeline(pipeline);
      pass.set_bind_group(2, &canvas.bind_group, &[]);
      pass.draw(0..3, 0..1);
    })
  }

  fn blit_pipeline(
    &self,
    gpu: &Gpu,
    format: wgpu::TextureFormat,
  ) -> wgpu::RenderPipeline {
    gpu
      .device()
      .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dovetail present"),
        layout: Some(&self.blit_layout),
        vertex: wgpu::VertexState {
          module: &self.shader,
          entry_point: Some("blit_vs"),
          compilation_options: Default::default(),
          buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
          module: &self.shader,
          entry_point: Some(if format.is_srgb() {
            "blit_fs"
          } else {
            "blit_encode_fs"
          }),
          compilation_options: Default::default(),
          targets: &[Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
          })],
        }),
        multiview_mask: None,
        cache: None,
      })
  }

  /// Reads the canvas back as sRGB bytes, red, green, blue and alpha for each
  /// pixel, row by row. Slow: for tests.
  pub fn read_pixels(
    &self,
    gpu: &Gpu,
    canvas: &Canvas,
  ) -> Vec<[u8; 4]> {
    let row = (canvas.width * 4).div_ceil(256) * 256;
    let buffer = gpu
      .device()
      .create_buffer(&wgpu::BufferDescriptor {
        label: Some("dovetail read back"),
        size: u64::from(row * canvas.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
      });
    let mut encoder =
      gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
          label: Some("dovetail read back"),
        });
    encoder.copy_texture_to_buffer(
      wgpu::TexelCopyTextureInfo {
        texture: &canvas.texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
      },
      wgpu::TexelCopyBufferInfo {
        buffer: &buffer,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(row),
          rows_per_image: None,
        },
      },
      wgpu::Extent3d {
        width: canvas.width,
        height: canvas.height,
        depth_or_array_layers: 1,
      },
    );
    gpu.queue().submit([encoder.finish()]);
    buffer
      .slice(..)
      .map_async(wgpu::MapMode::Read, |result| {
        result.expect("map read-back buffer")
      });
    gpu
      .device()
      .poll(wgpu::PollType::wait_indefinitely())
      .expect("wait for the GPU");
    let data = buffer
      .slice(..)
      .get_mapped_range()
      .expect("mapped range");
    let mut pixels =
      Vec::with_capacity((canvas.width * canvas.height) as usize);
    for y in 0..canvas.height {
      let start = (y * row) as usize;
      for x in 0..canvas.width as usize {
        let p = start + x * 4;
        pixels.push([
          data[p],
          data[p + 1],
          data[p + 2],
          data[p + 3],
        ]);
      }
    }
    pixels
  }
}

/// The key of a large icon drawn as an image: the icon, its size and
/// colour, apart from every image's own number.
fn large_icon_key(
  icon: u64,
  w: u32,
  h: u32,
  color: Color,
) -> u64 {
  use std::hash::{Hash, Hasher};
  let mut hash = std::collections::hash_map::DefaultHasher::new();
  (icon, w, h).hash(&mut hash);
  for v in [color.r, color.g, color.b, color.a] {
    v.to_bits().hash(&mut hash);
  }
  // Images count up from 1; these have the top bit set.
  hash.finish() | 1 << 63
}

trait WithAlpha {
  fn with_alpha(
    self,
    a: f32,
  ) -> Self;
}

impl WithAlpha for Color {
  fn with_alpha(
    mut self,
    a: f32,
  ) -> Self {
    self.a = a;
    self
  }
}

/// A colour in linear values, as the canvas blends them.
/// A logical rectangle in physical pixels, with its edges on whole pixels.
fn snap(
  r: Rect,
  scale: f32,
) -> [f32; 4] {
  let (x, y) = ((r.x * scale).round(), (r.y * scale).round());
  let right = ((r.x + r.width) * scale).round();
  let bottom = ((r.y + r.height) * scale).round();
  [x, y, right - x, bottom - y]
}

fn linear(c: Color) -> [f32; 4] {
  let l = |v: f32| srgb_to_linear(f64::from(v.clamp(0.0, 1.0))) as f32;
  [
    l(c.r),
    l(c.g),
    l(c.b),
    c.a.clamp(0.0, 1.0),
  ]
}

/// The whole pixels that a logical rectangle touches.
fn to_pixels(
  r: Rect,
  scale: f32,
) -> [u32; 4] {
  let x0 = (r.x * scale).floor().max(0.0);
  let y0 = (r.y * scale).floor().max(0.0);
  let x1 = ((r.x + r.width) * scale).ceil().max(x0);
  let y1 = ((r.y + r.height) * scale).ceil().max(y0);
  [
    x0 as u32,
    y0 as u32,
    (x1 - x0) as u32,
    (y1 - y0) as u32,
  ]
}

fn intersect(
  a: [u32; 4],
  b: [u32; 4],
) -> [u32; 4] {
  let x0 = a[0].max(b[0]);
  let y0 = a[1].max(b[1]);
  let x1 = (a[0] + a[2]).min(b[0] + b[2]);
  let y1 = (a[1] + a[3]).min(b[1] + b[3]);
  [
    x0,
    y0,
    x1.saturating_sub(x0),
    y1.saturating_sub(y0),
  ]
}

/// Draws one scrollbar (M15): the track and a wide thumb while the
/// pointer is over it, or only a thin thumb along its outer edge.
fn scrollbar(
  rects: &mut Vec<f32>,
  batches: &mut Vec<Batch>,
  bar: &dovetail_layout::Scrollbar,
  wide: bool,
  colors: dovetail_core::WindowStyle,
  scale: f32,
  region: [u32; 4],
) {
  let scissor = intersect(to_pixels(bar.clip, scale), region);
  if scissor[2] == 0 || scissor[3] == 0 {
    return;
  }
  // The thumb is inset from the track's sides, and thin when not wide.
  let inset = 2.0;
  let thickness = if wide {
    dovetail_layout::SCROLLBAR_WIDTH - 2.0 * inset
  } else {
    dovetail_layout::SCROLLBAR_THIN
  };
  let t = bar.thumb;
  let thumb = if bar.vertical {
    Rect::new(
      t.x + t.width - inset - thickness,
      t.y + inset,
      thickness,
      (t.height - 2.0 * inset).max(0.0),
    )
  } else {
    Rect::new(
      t.x + inset,
      t.y + t.height - inset - thickness,
      (t.width - 2.0 * inset).max(0.0),
      thickness,
    )
  };
  let mut shapes = vec![(thumb, colors.scrollbar_thumb, thickness / 2.0)];
  if wide {
    let r = bar.track;
    shapes.insert(0, (r, colors.scrollbar_track, r.width.min(r.height) / 2.0));
  }
  for (r, color, radius) in shapes {
    let index = (rects.len() / 16) as u32;
    rects.extend([
      r.x * scale,
      r.y * scale,
      r.width * scale,
      r.height * scale,
    ]);
    rects.extend(linear(color));
    rects.extend(linear(color));
    rects.extend([radius * scale, 0.0, 0.0, 0.0]);
    match batches.last_mut() {
      Some(b)
        if b.pipeline == Pipeline::Rect
          && b.scissor == scissor
          && b.end == index =>
      {
        b.end += 1
      },
      _ => batches.push(Batch {
        pipeline: Pipeline::Rect,
        scissor,
        start: index,
        end: index + 1,
      }),
    }
  }
}

fn overlaps(
  a: [u32; 4],
  b: [u32; 4],
) -> bool {
  let i = intersect(a, b);
  i[2] > 0 && i[3] > 0
}

fn bytes(values: &[f32]) -> Vec<u8> {
  values
    .iter()
    .flat_map(|v| v.to_ne_bytes())
    .collect()
}

fn instance_buffer(
  device: &wgpu::Device,
  label: &str,
  values: &[f32],
) -> wgpu::Buffer {
  device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some(label),
    contents: &bytes(values),
    usage: wgpu::BufferUsages::VERTEX,
  })
}
