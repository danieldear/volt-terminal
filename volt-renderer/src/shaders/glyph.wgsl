struct VIn  { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32> }
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> }

@group(0) @binding(0) var t_atlas: texture_2d<f32>;
@group(0) @binding(1) var s_atlas: sampler;

@vertex fn vs_glyph(in: VIn) -> VOut {
    return VOut(vec4<f32>(in.pos, 0.0, 1.0), in.uv, in.color);
}
@fragment fn fs_glyph(in: VOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_atlas, s_atlas, in.uv);
    return vec4<f32>(in.color.rgb * texel.rgb, in.color.a * texel.a);
}
