struct Params { amount: f32 }

// The blockbuster grade: shadows to teal, skin to warm amber, the rest a little drained, a moderate S-curve.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    let skin = skin_mask(c.rgb);
    let shade = pow(1.0 - clamp(luma(c.rgb), 0.0, 1.0), 2.0);
    var out = mix(saturation(c.rgb, 0.8), saturation(c.rgb, 1.15), skin);
    out = mix(out, out * vec3<f32>(0.55, 1.25, 1.4), shade * 0.35);
    out = mix(out, out * vec3<f32>(1.12, 1.0, 0.82), skin * 0.5);
    out = s_curve(out, 0.2);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
