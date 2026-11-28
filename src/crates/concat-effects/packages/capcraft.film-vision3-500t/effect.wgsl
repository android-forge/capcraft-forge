struct Params { amount: f32 }

// Night-interior stock: warm creamy highlights over cool blue-black shadows, soft roll-off, fine grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = film_curve(c.rgb, 0.15, 0.35);
    out = lift_gamma_gain(out, vec3<f32>(-0.012, 0.0, 0.03), vec3<f32>(1.0), vec3<f32>(1.04, 1.0, 0.94));
    out = saturation(out, 0.95);
    out = out + grain_at(uv, 0.015);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
