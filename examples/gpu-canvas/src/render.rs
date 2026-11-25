//! The board's own wgpu drawing: one instanced quad for each shape, shaded
//! by its distance (see `shapes.wgsl`).

use std::cell::RefCell;
use std::rc::Rc;

use dovetail::controls::Camera;
use dovetail::render::wgpu::util::DeviceExt;
use dovetail::render::{CanvasFrame, CanvasPainter, wgpu};

use crate::scene::{Kind, Scene};

/// Floats per shape: kind, size, selected, unused; two points; colour.
const FLOATS: usize = 12;

/// The board's background.
const BOARD: [f32; 4] = [0.08, 0.30, 0.16, 1.0];
/// A selection box being dragged: a light fill, with a ring.
const SELECTING: [f32; 4] = [0.12, 0.40, 0.96, 0.18];

/// What the GPU keeps between frames: the pipeline, and the shapes'
/// buffer while the scene and selection stay the same.
#[derive(Default)]
pub struct ShapeRenderer {
  gpu: RefCell<Option<Pipeline>>,
  shapes: RefCell<Option<Shapes>>,
}

struct Pipeline {
  generation: u64,
  format: wgpu::TextureFormat,
  pipeline: wgpu::RenderPipeline,
  layout: wgpu::BindGroupLayout,
}

struct Shapes {
  /// The scene and selection it was made from, and the device.
  key: (usize, usize, u64),
  buffer: wgpu::Buffer,
  count: u32,
}

/// sRGB to linear light, as the canvas texture wants.
fn linear(c: [f32; 4]) -> [f32; 4] {
  let l = |v: f32| {
    if v <= 0.04045 {
      v / 12.92
    } else {
      ((v + 0.055) / 1.055).powf(2.4)
    }
  };
  [l(c[0]), l(c[1]), l(c[2]), c[3]]
}

fn push(
  out: &mut Vec<f32>,
  kind: Kind,
  colour: [f32; 4],
  selected: bool,
) {
  let s = f32::from(u8::from(selected));
  let c = linear(colour);
  match kind {
    Kind::Pad { centre, half } => out.extend([
      0.0, 0.0, s, 0.0, centre.0, centre.1, half.0, half.1,
    ]),
    Kind::Via { centre, radius } => out.extend([
      1.0, radius, s, 0.0, centre.0, centre.1, 0.0, 0.0,
    ]),
    Kind::Trace { from, to, half_width } => out.extend([
      2.0, half_width, s, 0.0, from.0, from.1, to.0, to.1,
    ]),
  }
  out.extend(c);
}

/// The rectangle `(x0, y0, x1, y1)` as a pad.
fn rect_kind((x0, y0, x1, y1): (f32, f32, f32, f32)) -> Kind {
  Kind::Pad {
    centre: ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
    half: ((x1 - x0) / 2.0, (y1 - y0) / 2.0),
  }
}

impl ShapeRenderer {
  fn pipeline(
    &self,
    frame: &CanvasFrame<'_>,
  ) {
    let mut gpu = self.gpu.borrow_mut();
    if gpu.as_ref().is_some_and(|g| {
      g.generation == frame.generation && g.format == frame.format
    }) {
      return;
    }
    let device = frame.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("board shapes"),
      source: wgpu::ShaderSource::Wgsl(include_str!("shapes.wgsl").into()),
    });
    let layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("board view"),
        entries: &[wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        }],
      });
    let pipeline_layout =
      device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("board shapes"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
      });
    let attributes = [
      wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
      },
      wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
      },
      wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
      },
    ];
    let pipeline =
      device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("board shapes"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
          module: &shader,
          entry_point: Some("vs"),
          compilation_options: Default::default(),
          buffers: &[Some(wgpu::VertexBufferLayout {
            array_stride: (FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attributes,
          })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
          module: &shader,
          entry_point: Some("fs"),
          compilation_options: Default::default(),
          targets: &[Some(wgpu::ColorTargetState {
            format: frame.format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
          })],
        }),
        multiview_mask: None,
        cache: None,
      });
    *gpu = Some(Pipeline {
      generation: frame.generation,
      format: frame.format,
      pipeline,
      layout,
    });
  }
}

/// Draws the board as it is now. A new one is made for each change, which
/// repaints the canvas; the renderer keeps what can be kept.
pub struct BoardPainter {
  /// Kept between frames.
  pub renderer: Rc<ShapeRenderer>,
  /// The shapes.
  pub scene: Rc<Scene>,
  /// Which shapes are selected, by index.
  pub selected: Rc<Vec<bool>>,
  /// Where the view looks.
  pub camera: Camera,
  /// A selection box being dragged, in world units.
  pub selecting: Option<(f32, f32, f32, f32)>,
}

impl CanvasPainter for BoardPainter {
  fn paint(
    &self,
    frame: &mut CanvasFrame<'_>,
  ) {
    self.renderer.pipeline(frame);
    let gpu = self.renderer.gpu.borrow();
    let Some(gpu) = gpu.as_ref() else {
      return;
    };
    let device = frame.device;

    // The shapes, made again only when the scene or selection changed.
    let key = (
      Rc::as_ptr(&self.scene) as usize,
      Rc::as_ptr(&self.selected) as usize,
      frame.generation,
    );
    let mut shapes = self.renderer.shapes.borrow_mut();
    if shapes.as_ref().is_none_or(|s| s.key != key) {
      let mut data = Vec::with_capacity((self.scene.shapes.len() + 1) * FLOATS);
      push(&mut data, rect_kind(self.scene.board), BOARD, false);
      for (i, shape) in self.scene.shapes.iter().enumerate() {
        let selected = self.selected.get(i).copied().unwrap_or(false);
        push(&mut data, shape.kind, shape.colour, selected);
      }
      let buffer =
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
          label: Some("board shapes"),
          contents: &bytes(&data),
          usage: wgpu::BufferUsages::VERTEX,
        });
      *shapes = Some(Shapes {
        key,
        buffer,
        count: (data.len() / FLOATS) as u32,
      });
    }
    let shapes = shapes.as_ref().expect("made above");

    let [_, _, w, h] = frame.viewport;
    let c = self.camera;
    let view = [
      c.origin.0,
      c.origin.1,
      c.zoom,
      frame.scale,
      w,
      h,
      0.0,
      0.0,
    ];
    let uniform =
      device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("board view"),
        contents: &bytes(&view),
        usage: wgpu::BufferUsages::UNIFORM,
      });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("board view"),
      layout: &gpu.layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform.as_entire_binding(),
      }],
    });
    let overlay = self.selecting.map(|r| {
      let mut data = Vec::with_capacity(FLOATS);
      push(&mut data, rect_kind(r), SELECTING, true);
      device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("selection box"),
        contents: &bytes(&data),
        usage: wgpu::BufferUsages::VERTEX,
      })
    });

    let mut pass = frame.begin("board");
    pass.set_pipeline(&gpu.pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, shapes.buffer.slice(..));
    pass.draw(0..6, 0..shapes.count);
    if let Some(overlay) = &overlay {
      pass.set_vertex_buffer(0, overlay.slice(..));
      pass.draw(0..6, 0..1);
    }
  }
}

/// Floats as little-endian bytes, for a buffer.
fn bytes(values: &[f32]) -> Vec<u8> {
  values
    .iter()
    .flat_map(|v| v.to_le_bytes())
    .collect()
}
