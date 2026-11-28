struct Params { amount: f32 }

// CineStill nights: deep blue-black shadows and red-orange bloom round every practical light.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = white_balance(c.rgb, 7800.0);
    out = lift_gamma_gain(out, vec3<f32>(0.0, 0.004, 0.03), vec3<f32>(1.0), vec3<f32>(1.0));
    out = halation(uv, out, 0.6, 9.0, vec3<f32>(1.0, 0.35, 0.1), 0.95);
    out = film_curve(out, 0.2, 0.2);
    out = out + grain_at(uv, 0.02);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
