struct Params { amount: f32 }

// Pushed black and white: hard contrast, pure blacks and heavy grain.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    var out = mono(c.rgb, vec3<f32>(0.3, 0.59, 0.11));
    out = s_curve(out, 0.5);
    out = out + vec3<f32>(grain_at(uv, 0.06).r);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
