struct Params { amount: f32 }

// Daylight stock: neutral, rich colour, a moderate shoulder and tight grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = film_curve(c.rgb, 0.1, 0.25);
    out = saturation(out, 1.15);
    out = out + grain_at(uv, 0.008);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
