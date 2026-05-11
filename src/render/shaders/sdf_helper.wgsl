// P0.5.2 — SDF sampling helpers.
//
// Consumers prepend this file at pipeline build time (via Rust string
// concatenation in `SDF_HELPER_WGSL`) and pass their `texture_2d<f32>`
// SDF binding into the functions explicitly. No global bindings are
// declared here — consumers pick their own bind slots.
//
// The math mirrors warp.wgsl's prior inline `sample_sdf_bilinear`
// exactly: textureLoad (R32Float is unfilterable) at the four
// surrounding texels, then a bilinear mix.
//
// NOTE: `dyn` is a WGSL reserved identifier; gradient temporaries are
// named `dy_p` / `dy_n` to avoid parse errors.

fn sample_sdf_bilinear(t_sdf: texture_2d<f32>, uv: vec2<f32>) -> f32 {
    let dims_u = textureDimensions(t_sdf);
    let dims = vec2<f32>(dims_u);
    let max_x = i32(dims_u.x) - 1;
    let max_y = i32(dims_u.y) - 1;
    let p = uv * dims - vec2<f32>(0.5);
    let i0 = vec2<i32>(i32(floor(p.x)), i32(floor(p.y)));
    let f = p - vec2<f32>(f32(i0.x), f32(i0.y));
    let ix0 = clamp(i0.x, 0, max_x);
    let iy0 = clamp(i0.y, 0, max_y);
    let ix1 = clamp(i0.x + 1, 0, max_x);
    let iy1 = clamp(i0.y + 1, 0, max_y);
    let v00 = textureLoad(t_sdf, vec2<i32>(ix0, iy0), 0).r;
    let v10 = textureLoad(t_sdf, vec2<i32>(ix1, iy0), 0).r;
    let v01 = textureLoad(t_sdf, vec2<i32>(ix0, iy1), 0).r;
    let v11 = textureLoad(t_sdf, vec2<i32>(ix1, iy1), 0).r;
    return mix(mix(v00, v10, f.x), mix(v01, v11, f.x), f.y);
}

// Finite-difference gradient in normalized-UV space. Returns
// (df/dx, df/dy) where x and y are in UV units (same scale as the
// `uv` input). Per-axis epsilon is one texel; central differences
// give second-order accuracy.
fn sample_sdf_gradient(t_sdf: texture_2d<f32>, uv: vec2<f32>) -> vec2<f32> {
    let dims = vec2<f32>(textureDimensions(t_sdf));
    let eps = vec2<f32>(1.0) / dims;
    let dx_p = sample_sdf_bilinear(t_sdf, uv + vec2<f32>(eps.x, 0.0));
    let dx_n = sample_sdf_bilinear(t_sdf, uv - vec2<f32>(eps.x, 0.0));
    let dy_p = sample_sdf_bilinear(t_sdf, uv + vec2<f32>(0.0, eps.y));
    let dy_n = sample_sdf_bilinear(t_sdf, uv - vec2<f32>(0.0, eps.y));
    return vec2<f32>((dx_p - dx_n) / (2.0 * eps.x), (dy_p - dy_n) / (2.0 * eps.y));
}

// Convenience: returns (distance, gradient.x, gradient.y) in one call.
fn sample_sdf(t_sdf: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
    let d = sample_sdf_bilinear(t_sdf, uv);
    let g = sample_sdf_gradient(t_sdf, uv);
    return vec3<f32>(d, g.x, g.y);
}
