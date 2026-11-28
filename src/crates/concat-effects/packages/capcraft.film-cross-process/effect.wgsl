struct Params { amount: f32 }

// Slide film in negative chemistry: magenta shadows, green highlights, loud colour.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = lift_gamma_gain(c.rgb, vec3<f32>(0.04, -0.01, 0.03), vec3<f32>(1.0), vec3<f32>(0.97, 1.06, 0.92));
    out = saturation(out, 1.2);
    out = s_curve(out, 0.3);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
