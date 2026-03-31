struct VIn  { @location(0) pos: vec2<f32>, @location(1) color: vec4<f32> }
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> }

@vertex fn vs_bg(in: VIn) -> VOut {
    return VOut(vec4<f32>(in.pos, 0.0, 1.0), in.color);
}
@fragment fn fs_bg(in: VOut) -> @location(0) vec4<f32> { return in.color; }
