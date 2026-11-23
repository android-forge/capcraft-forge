struct Params { x: f32, y: f32, size: f32, stops: f32 }

// A round of light on the picture, as a reflector or a small lamp held near
// a face gives: strongest at its middle and falling away softly, a round on
// any frame's shape. It brightens what is dark and middling and leaves what
// is already bright, in light: a white shirt or a lamp in it stays as it was.

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let aspect = frame.size.x / max(frame.size.y, 1.0);
    let p = (uv - vec2<f32>(params.x, params.y) / 100.0) * vec2<f32>(aspect, 1.0);
    let r = max(params.size / 100.0, 0.01);
    let near = exp(-dot(p, p) / (2.0 * r * r));
    let keep = smoothstep(0.1, 0.55, luma(max(c.rgb, vec3<f32>(0.0))));
    let gain = exp2(params.stops * near * (1.0 - keep));
    return vec4<f32>(c.rgb * gain, c.a);
}
