struct Params { amount: f32 }

// Daylight shot played as night: a stop and more down, cooled, blues drained, shadows crushed.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = c.rgb * 0.4;
    out = white_balance(out, 11000.0);
    out = saturation(out, 0.5);
    out = film_curve(out, 0.4, 0.0);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
