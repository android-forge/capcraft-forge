// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Every effect drawn on a frame, for looking at beside `fx.py`'s own.
//!
//! `FXREF` names a folder holding `frame.rgb` (raw RGB), `mask.u8` (one
//! byte a pixel) and `inputs.json` (`{"w", "h", "face": [x, y, w]}` in
//! pixels); each effect is drawn at 0.5, 1.5 and 2.5 s of a 3 s run and
//! written beside them as `<name>@<t>.rs.rgb`.

use std::collections::BTreeMap;

fn number_after(json: &str, key: &str) -> f32 {
    let at = json.find(key).expect("the key") + key.len();
    let rest = json[at..].trim_start_matches(|c: char| c == '"' || c == ':' || c.is_whitespace() || c == '[');
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().expect("a number")
}

#[test]
#[ignore = "needs the reference frame"]
fn every_effect_draws_beside_the_reference() {
    let Ok(dir) = std::env::var("FXREF") else { return };
    let dir = std::path::Path::new(&dir);
    let json = std::fs::read_to_string(dir.join("inputs.json")).unwrap();
    let (w, h) = (number_after(&json, "\"w\"") as usize, number_after(&json, "\"h\"") as usize);
    let face_at = json.find("\"face\"").unwrap();
    let nums: Vec<f32> = json[face_at..]
        .split(|c: char| c == '[' || c == ']' || c == ',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let rgb = std::fs::read(dir.join("frame.rgb")).unwrap();
    let mask: Vec<f32> = std::fs::read(dir.join("mask.u8")).unwrap().iter().map(|&v| f32::from(v) / 255.0).collect();
    let face = capcraft_reelfx::Face { x: nums[0] / w as f32, y: nums[1] / h as f32, w: nums[2] / w as f32 };
    let mut thumb = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
    for (px, c) in thumb.data_mut().chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
        px.copy_from_slice(&[c[0], c[1], c[2], 255]);
    }
    let only = std::env::var("FXONLY").ok();
    for effect in capcraft_reelfx::effects() {
        if only.as_deref().is_some_and(|o| !o.split(',').any(|n| n == effect.name)) {
            continue;
        }
        let mut run = capcraft_reelfx::Run::new(effect.name, 0.0, 3.0, BTreeMap::new()).unwrap();
        run.cards = std::sync::Arc::new(vec![thumb.clone(); 4]);
        let mut runs = [run];
        for t in [0.5f32, 1.5, 2.5] {
            let mut rgba: Vec<u8> = rgb.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
            let started = std::time::Instant::now();
            capcraft_reelfx::draw(&mut rgba, w, h, Some(&mask), Some(face), &mut runs, t);
            let ms = started.elapsed().as_millis();
            let out: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            std::fs::write(dir.join(format!("{}@{t}.rs.rgb", effect.name)), out).unwrap();
            println!("{} @{t}: {ms} ms", effect.name);
        }
    }
}
