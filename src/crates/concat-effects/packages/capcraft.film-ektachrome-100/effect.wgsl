struct Params { amount: f32 }

// Slide film: punchy reds and greens, high contrast, cool neutral blacks, highlights that clip hard.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = s_curve(c.rgb, 0.35);
    out = saturation(out, 1.3);
    out = lift_gamma_gain(out, vec3<f32>(0.0, 0.0, 0.01), vec3<f32>(1.0), vec3<f32>(1.04));
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
