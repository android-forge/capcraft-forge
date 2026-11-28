struct Params { amount: f32 }

// Soft romance stock: creamy highlights, pastel colour, warm brown-black shadows, organic grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = lift_gamma_gain(c.rgb, vec3<f32>(0.025, 0.014, 0.0), vec3<f32>(1.0), vec3<f32>(1.0));
    out = film_curve(out, 0.05, 0.5);
    out = saturation(out, 0.8);
    out = out + grain_at(uv, 0.012);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
