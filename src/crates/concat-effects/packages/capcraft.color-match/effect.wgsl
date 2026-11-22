struct Params {
    strength: f32,
    light_gain: f32,
    light_shift: f32,
    blue_gain: f32,
    blue_shift: f32,
    red_gain: f32,
    red_shift: f32,
}

// The picture as light and two colour differences, blue and red less the
// light, each scaled and shifted, and put back together. Nothing goes below
// black that was not below it already.
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let light = dot(c.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let y = light * params.light_gain + params.light_shift;
    let u = (c.b - light) * params.blue_gain + params.blue_shift;
    let v = (c.r - light) * params.red_gain + params.red_shift;
    let r = y + v;
    let b = y + u;
    let g = (y - 0.2126 * r - 0.0722 * b) / 0.7152;
    let matched = max(vec3<f32>(r, g, b), min(c.rgb, vec3<f32>(0.0)));
    return vec4<f32>(mix(c.rgb, matched, params.strength / 100.0), c.a);
}
