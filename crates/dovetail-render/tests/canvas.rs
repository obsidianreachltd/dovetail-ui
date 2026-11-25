//! M09 "Done when" items 1 and 2: a canvas draws with its own wgpu render
//! pass inside the normal frame, only inside its clip, under what is drawn
//! after it, and again when its drawing changes.
//!
//! Needs a GPU adapter, so it does not run by default:
//! `cargo test -p dovetail-render --test canvas -- --ignored`.

use std::cell::Cell;

use dovetail_core::{
  BuildContext, Color, Component, Edges, Element, ElementKind, Signal, Size,
  Style, Tree, View,
};
use dovetail_layout::{Layout, Rect};
use dovetail_render::{
  CanvasFrame, CanvasPainter, Gpu, PaintInput, Painter, canvas, font_measure,
  wgpu,
};

/// Fills its whole viewport with one colour, from a pipeline of its own.
struct Fill([f64; 4]);

impl CanvasPainter for Fill {
  fn paint(
    &self,
    frame: &mut CanvasFrame<'_>,
  ) {
    let shader = frame
      .device
      .create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fill"),
        source: wgpu::ShaderSource::Wgsl(
          r#"
          @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            let x = f32(i32(i & 1u) * 4 - 1);
            let y = f32(i32(i >> 1u) * 4 - 1);
            return vec4<f32>(x, y, 0.0, 1.0);
          }
          struct C { c: vec4<f32> }
          @group(0) @binding(0) var<uniform> colour: C;
          @fragment fn fs() -> @location(0) vec4<f32> { return colour.c; }
          "#
          .into(),
        ),
      });
    let layout =
      frame
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
          label: None,
          entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
              ty: wgpu::BufferBindingType::Uniform,
              has_dynamic_offset: false,
              min_binding_size: None,
            },
            count: None,
          }],
        });
    let pipeline =
      frame
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
          label: Some("fill"),
          layout: Some(&frame.device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
              label: None,
              bind_group_layouts: &[Some(&layout)],
              immediate_size: 0,
            },
          )),
          vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
          },
          primitive: wgpu::PrimitiveState::default(),
          depth_stencil: None,
          multisample: wgpu::MultisampleState::default(),
          fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(frame.format.into())],
          }),
          multiview_mask: None,
          cache: None,
        });
    let colour: Vec<u8> = self
      .0
      .iter()
      .flat_map(|v| (*v as f32).to_le_bytes())
      .collect();
    let buffer = frame
      .device
      .create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      });
    frame.queue.write_buffer(&buffer, 0, &colour);
    let group = frame
      .device
      .create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
          binding: 0,
          resource: buffer.as_entire_binding(),
        }],
      });
    let mut pass = frame.begin("fill");
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.draw(0..3, 0..1);
  }
}

thread_local! {
  static SCENE: Cell<Option<Signal<View>>> = const { Cell::new(None) };
}

struct Scene;

#[derive(PartialEq)]
struct SceneProps(View);

impl Component for Scene {
  type Props = SceneProps;

  fn build(
    props: &SceneProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let first = props.0.clone();
    let view = cx.use_state(move || first);
    SCENE.set(Some(view));
    view.get()
  }
}

fn fixed(
  w: f32,
  h: f32,
) -> Style {
  Style {
    width: Size::Fixed(w),
    height: Size::Fixed(h),
    ..Style::default()
  }
}

/// A 50 × 50 clipping box at (10, 10) holding a 100 × 100 canvas, and a
/// 20 × 20 blue box at (30, 30) drawn after it.
fn scene(colour: [f64; 4]) -> View {
  let clipped: View = Element::new(ElementKind::Box)
    .style(Style { clip: true, ..fixed(50.0, 50.0) })
    .children([Element::new(ElementKind::Box)
      .style(fixed(100.0, 100.0))
      .canvas(canvas(Fill(colour)))
      .into()])
    .into();
  let blue: View = Element::new(ElementKind::Box)
    .style(Style {
      background: Color::rgb(0.0, 0.0, 1.0),
      ..fixed(20.0, 20.0)
    })
    .into();
  Element::new(ElementKind::Stack)
    .children([
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::all(10.0),
          ..Style::default()
        })
        .children([clipped])
        .into(),
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::all(30.0),
          ..Style::default()
        })
        .children([blue])
        .into(),
    ])
    .into()
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_canvas_draws_its_own_pass_inside_its_clip_and_in_order() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let mut target = painter.canvas(&gpu, 100, 100);
  let mut tree = Tree::new();
  tree.mount(View::component::<Scene>(SceneProps(scene([
    1.0, 0.0, 0.0, 1.0,
  ]))));
  let mut layout = Layout::new();
  let mut frame =
    |tree: &mut Tree, layout: &mut Layout, painter: &mut Painter| {
      tree.update();
      let changes = layout.update(tree, 100.0, 100.0, &font_measure());
      painter.paint(
        &gpu,
        &mut target,
        &PaintInput {
          region: changes.damage_bounds().or(Some(Rect::default())),
          ..PaintInput::new(layout.boxes(), 1.0)
        },
      );
      painter.read_pixels(&gpu, &target)
    };
  let at = |p: &[[u8; 4]], x: usize, y: usize| p[y * 100 + x];

  let p = frame(&mut tree, &mut layout, &mut painter);
  assert_eq!(at(&p, 20, 20), [255, 0, 0, 255], "the canvas draws");
  assert_eq!(at(&p, 55, 15), [255, 0, 0, 255]);
  assert_eq!(at(&p, 65, 20), [255, 255, 255, 255], "not outside its clip");
  assert_eq!(at(&p, 20, 65), [255, 255, 255, 255]);
  assert_eq!(at(&p, 5, 5), [255, 255, 255, 255], "not before its box");
  assert_eq!(
    at(&p, 40, 40),
    [0, 0, 255, 255],
    "under what comes after it"
  );

  // New drawing: the canvas is painted again, and the rest stays.
  SCENE
    .get()
    .unwrap()
    .set(scene([0.0, 1.0, 0.0, 1.0]));
  let p = frame(&mut tree, &mut layout, &mut painter);
  assert_eq!(at(&p, 20, 20), [0, 255, 0, 255]);
  assert_eq!(at(&p, 40, 40), [0, 0, 255, 255]);
  assert_eq!(at(&p, 65, 20), [255, 255, 255, 255]);
}
