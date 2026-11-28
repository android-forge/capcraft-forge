struct Params { amount: f32 }

// Nostalgic stills: warm golden roll-off, brown-black shadows, rich colour that is not overdone, fine grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = white_balance(c.rgb, 5200.0);
    out = lift_gamma_gain(out, vec3<f32>(0.02, 0.012, 0.0), vec3<f32>(1.0), vec3<f32>(1.0));
    out = film_curve(out, 0.08, 0.45);
    out = saturation(out, 1.05);
    out = out + grain_at(uv, 0.012);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
