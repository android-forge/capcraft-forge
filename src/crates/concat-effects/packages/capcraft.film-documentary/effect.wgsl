struct Params { amount: f32 }

// Desaturated documentary: colour down by a third, lifted blacks, skin kept, fine 16 mm texture.
// Mixed in by the amount, so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let k = params.amount / 100.0;
    let skin = skin_mask(c.rgb);
    var out = mix(saturation(c.rgb, 0.68), saturation(c.rgb, 0.92), skin);
    out = matte(out, 0.05, 0.97);
    out = out + grain_at(uv, 0.01);
    return vec4<f32>(mix(c.rgb, out, k), c.a);
}
