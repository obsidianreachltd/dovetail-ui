// Pads, vias and traces, each drawn as a quad round its bounds, coloured by
// its signed distance, with anti-aliased edges and a ring when selected.

struct View {
  origin: vec2<f32>, // the world point at the canvas's top-left corner
  zoom: f32,         // logical pixels per world unit
  scale: f32,        // physical pixels per logical pixel
  size: vec2<f32>,   // the canvas in physical pixels
  _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

struct Shape {
  @location(0) info: vec4<f32>,   // kind, size, selected, unused
  @location(1) points: vec4<f32>, // a.x, a.y, b.x, b.y
  @location(2) colour: vec4<f32>, // linear
};

struct Out {
  @builtin(position) position: vec4<f32>,
  @location(0) world: vec2<f32>,
  @location(1) @interpolate(flat) info: vec4<f32>,
  @location(2) @interpolate(flat) points: vec4<f32>,
  @location(3) @interpolate(flat) colour: vec4<f32>,
};

const HIGHLIGHT: vec3<f32> = vec3<f32>(1.0, 0.93, 0.35);

fn corner(i: u32) -> vec2<f32> {
  let c = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
  );
  return c[i];
}

@vertex
fn vs(@builtin(vertex_index) index: u32, s: Shape) -> Out {
  let kind = s.info.x;
  let size = s.info.y;
  let a = s.points.xy;
  let b = s.points.zw;
  var lo: vec2<f32>;
  var hi: vec2<f32>;
  if (kind < 0.5) {         // pad: centre a, half size b
    lo = a - b; hi = a + b;
  } else if (kind < 1.5) {  // via: centre a, radius size
    lo = a - vec2<f32>(size); hi = a + vec2<f32>(size);
  } else {                  // trace: from a to b, half width size
    lo = min(a, b) - vec2<f32>(size); hi = max(a, b) + vec2<f32>(size);
  }
  // Room for the soft edge and the selection ring, four pixels.
  let pixel = 1.0 / (view.zoom * view.scale);
  lo -= vec2<f32>(4.0 * pixel);
  hi += vec2<f32>(4.0 * pixel);
  let world = mix(lo, hi, corner(index));
  let px = (world - view.origin) * view.zoom * view.scale;
  var out: Out;
  out.position = vec4<f32>(px.x / view.size.x * 2.0 - 1.0, 1.0 - px.y / view.size.y * 2.0, 0.0, 1.0);
  out.world = world;
  out.info = s.info;
  out.points = s.points;
  out.colour = s.colour;
  return out;
}

@fragment
fn fs(v: Out) -> @location(0) vec4<f32> {
  let kind = v.info.x;
  let size = v.info.y;
  let a = v.points.xy;
  let b = v.points.zw;
  let p = v.world;
  var d: f32;
  if (kind < 0.5) {
    let q = abs(p - a) - b;
    d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
  } else if (kind < 1.5) {
    d = length(p - a) - size;
  } else {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-9), 0.0, 1.0);
    d = length(pa - ba * h) - size;
  }
  // The distance in physical pixels.
  let px = d * view.zoom * view.scale;
  let fill = clamp(0.5 - px, 0.0, 1.0);
  let selected = v.info.z > 0.5;
  var rgb = v.colour.rgb;
  if (selected) {
    rgb = mix(rgb, HIGHLIGHT, 0.45);
  }
  // A ring up to three pixels outside a selected shape.
  var ring = 0.0;
  if (selected) {
    ring = clamp(3.5 - px, 0.0, 1.0) * (1.0 - fill);
  }
  let alpha = fill * v.colour.a + ring;
  if (alpha <= 0.0) {
    discard;
  }
  let colour = (rgb * fill * v.colour.a + HIGHLIGHT * ring) / alpha;
  return vec4<f32>(colour, alpha);
}
