// Shaders for Dovetail's painter. Positions are in physical pixels, with
// (0, 0) at the top left.

struct Globals {
  size: vec2<f32>,
  _unused: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

fn corner(index: u32) -> vec2<f32> {
  var corners = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
  );
  return corners[index];
}

fn to_clip(p: vec2<f32>) -> vec4<f32> {
  return vec4<f32>(p / globals.size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

// --- Rectangles with rounded corners and borders ---------------------------

struct RectIn {
  @location(0) rect: vec4<f32>,         // x, y, width, height
  @location(1) color: vec4<f32>,        // linear, not premultiplied
  @location(2) border_color: vec4<f32>,
  @location(3) params: vec4<f32>,       // radius, border width
};

struct RectOut {
  @builtin(position) position: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) half_size: vec2<f32>,
  @location(2) color: vec4<f32>,
  @location(3) border_color: vec4<f32>,
  @location(4) params: vec4<f32>,
};

@vertex
fn rect_vs(@builtin(vertex_index) index: u32, r: RectIn) -> RectOut {
  let c = corner(index);
  var out: RectOut;
  out.position = to_clip(r.rect.xy + c * r.rect.zw);
  out.local = (c - vec2<f32>(0.5, 0.5)) * r.rect.zw;
  out.half_size = r.rect.zw * 0.5;
  out.color = r.color;
  out.border_color = r.border_color;
  out.params = r.params;
  return out;
}

@fragment
fn rect_fs(v: RectOut) -> @location(0) vec4<f32> {
  let radius = min(v.params.x, min(v.half_size.x, v.half_size.y));
  // Signed distance to the rounded box: negative inside.
  let q = abs(v.local) - v.half_size + vec2<f32>(radius, radius);
  let distance = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
  let outside = clamp(distance + 0.5, 0.0, 1.0);
  let border_width = v.params.y;
  var color = v.color;
  if (border_width > 0.0) {
    let in_border = clamp(distance + border_width + 0.5, 0.0, 1.0);
    color = mix(v.color, v.border_color, in_border);
  }
  return vec4<f32>(color.rgb, color.a * (1.0 - outside));
}

// --- Text from the glyph atlas ---------------------------------------------

@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct GlyphIn {
  @location(0) rect: vec4<f32>,  // x, y, width, height in pixels
  @location(1) uv: vec4<f32>,    // u, v, width, height in the atlas (0 to 1)
  @location(2) color: vec4<f32>,
  @location(3) backdrop: vec4<f32>, // what the text is drawn over, linear
};

struct GlyphOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) backdrop: vec4<f32>,
};

@vertex
fn glyph_vs(@builtin(vertex_index) index: u32, g: GlyphIn) -> GlyphOut {
  let c = corner(index);
  var out: GlyphOut;
  out.position = to_clip(g.rect.xy + c * g.rect.zw);
  out.uv = g.uv.xy + c * g.uv.zw;
  out.color = g.color;
  out.backdrop = g.backdrop;
  return out;
}

// One sRGB value as linear light.
fn srgb_to_linear_1(c: f32) -> f32 {
  if (c <= 0.04045) {
    return c / 12.92;
  }
  return pow((c + 0.055) / 1.055, 2.4);
}

@fragment
fn glyph_fs(v: GlyphOut) -> @location(0) vec4<f32> {
  let coverage = textureSample(atlas, atlas_sampler, v.uv).r;
  // Text weight. The canvas blends in linear light, which makes the soft
  // edges of dark text on a light background look too light, and of light
  // text on a dark background too heavy. So the coverage is changed to
  // give, in brightness, the result of blending in sRGB, as text renderers
  // usually do, for this text colour over this backdrop.
  let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
  let f = dot(v.color.rgb, luma);
  let b = dot(v.backdrop.rgb, luma);
  var weight = coverage;
  if (abs(f - b) > 0.001) {
    let fs = linear_to_srgb(vec3<f32>(f)).x;
    let bs = linear_to_srgb(vec3<f32>(b)).x;
    let wanted = srgb_to_linear_1(mix(bs, fs, coverage));
    weight = clamp((wanted - b) / (f - b), 0.0, 1.0);
  }
  return vec4<f32>(v.color.rgb, v.color.a * weight);
}

// --- Images (M17) -----------------------------------------------------------

// Each image is a texture of its own in group 1, with premultiplied pixels
// stored as sRGB, so sampling gives premultiplied linear light (D0023).

struct ImageIn {
  @location(0) rect: vec4<f32>,  // x, y, width, height in pixels
  @location(1) uv: vec4<f32>,    // u, v, width, height in the texture (0 to 1)
  @location(2) params: vec4<f32>, // opacity
};

struct ImageOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) opacity: f32,
};

@vertex
fn image_vs(@builtin(vertex_index) index: u32, i: ImageIn) -> ImageOut {
  let c = corner(index);
  var out: ImageOut;
  out.position = to_clip(i.rect.xy + c * i.rect.zw);
  out.uv = i.uv.xy + c * i.uv.zw;
  out.opacity = i.params.x;
  return out;
}

@fragment
fn image_fs(v: ImageOut) -> @location(0) vec4<f32> {
  return textureSample(atlas, atlas_sampler, v.uv) * v.opacity;
}

// --- Copying the canvas to the window --------------------------------------

@group(2) @binding(0) var canvas: texture_2d<f32>;
@group(2) @binding(1) var canvas_sampler: sampler;

struct BlitOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn blit_vs(@builtin(vertex_index) index: u32) -> BlitOut {
  // One triangle that covers the whole window.
  let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  var out: BlitOut;
  out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
  out.uv = uv;
  return out;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
  let low = c * 12.92;
  let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
  return select(high, low, c <= vec3<f32>(0.0031308));
}

// For an sRGB window, which converts by itself.
@fragment
fn blit_fs(v: BlitOut) -> @location(0) vec4<f32> {
  return textureSample(canvas, canvas_sampler, v.uv);
}

// For any other window, which needs sRGB values.
@fragment
fn blit_encode_fs(v: BlitOut) -> @location(0) vec4<f32> {
  let color = textureSample(canvas, canvas_sampler, v.uv);
  return vec4<f32>(linear_to_srgb(color.rgb), color.a);
}
