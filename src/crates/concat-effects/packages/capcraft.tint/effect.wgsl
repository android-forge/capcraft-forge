struct Params { color: vec4<f32>, amount: f32 }

// Each pixel takes the tint's colour at its own brightness: dark stays
// dark and light light, so a face keeps its shape. Mixed in by the amount
// (and the colour's own alpha), so none of it is as shot.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
    let lum = dot(c.rgb, weights);
    let tone = max(dot(params.color.rgb, weights), 0.05);
    // the colour scaled to the pixel's brightness, a little of the white kept in the highlights
    let washed = clamp(params.color.rgb * (lum / tone), vec3<f32>(0.0), vec3<f32>(1.0));
    let tinted = mix(washed, vec3<f32>(lum), smoothstep(0.75, 1.0, lum) * 0.5);
    let k = clamp(params.amount / 100.0, 0.0, 1.0) * params.color.a;
    return vec4<f32>(mix(c.rgb, tinted, k), c.a);
}
