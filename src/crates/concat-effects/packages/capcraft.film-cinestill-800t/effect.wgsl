struct Params { amount: f32 }

// Tungsten night stock: cool light, warm blacks and red halation blooming round the highlights.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = white_balance(c.rgb, 7200.0);
    out = lift_gamma_gain(out, vec3<f32>(0.015, 0.005, 0.0), vec3<f32>(1.0), vec3<f32>(1.0));
    out = halation(uv, out, 0.72, 6.0, vec3<f32>(1.0, 0.28, 0.12), 0.6);
    out = out + grain_at(uv, 0.02);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
