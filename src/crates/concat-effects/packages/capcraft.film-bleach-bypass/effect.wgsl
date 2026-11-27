struct Params { amount: f32 }

// The war look: colour drained by a third, an aggressive S-curve, crushed metallic shadows and pushed grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    let silver = mono(c.rgb, vec3<f32>(0.3, 0.59, 0.11));
    var out = mix(saturation(c.rgb, 0.6), silver * c.rgb * 2.0, 0.35);
    out = s_curve(out, 0.6);
    out = halation(uv, out, 0.9, 3.0, vec3<f32>(1.0), 0.2);
    out = out + vec3<f32>(grain_at(uv, 0.04).r);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
