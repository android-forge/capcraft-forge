// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Built-in templates: reel looks laid out on a fresh project with places
//! for the user's own photos and videos, titles to be typed over, graphics
//! and effects already set. The places are filled as a reel template's are
//! ([`crate::reel`]), each keeping the place's framing, keys and effects.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, micros};

/// Where the timeline keeps what a template's places want; the same key a
/// reel's template uses, so Replace media fills both alike.
const REEL: &str = "capcraftReel";

/// A modern reel's ease: quick away and a long settle.
const SETTLE: [f64; 4] = [0.16, 1.0, 0.3, 1.0];
/// Even in and out.
const EVEN: [f64; 4] = [0.42, 0.0, 0.58, 1.0];

/// What one place wants.
#[derive(Default, Clone)]
struct Place {
    /// `contain` (all of the picture seen) or `cover` (the frame filled).
    fit: &'static str,
    /// Its size on top of that.
    k: f64,
    x: f64,
    y: f64,
    rotation: f64,
    /// Keys: property, where (0 to 1), value; a scale's value is a share
    /// of the place's own size, offsets and the rest as they are.
    keys: Vec<(&'static str, f64, f64)>,
    /// Effects the picture in it carries.
    fx: Vec<Value>,
    /// A video put here plays backwards.
    reverse: bool,
    /// Mirrored left to right.
    flip: bool,
    /// A video here runs fast, slow and fast again, its length kept.
    ramp: bool,
    /// A copy of the main place at this index: filled with whatever it is
    /// filled with, and not offered to be filled on its own.
    copy_of: Option<usize>,
    /// How it lays over what is under it ("screen" and the rest); empty is normal.
    blend: &'static str,
    /// A mask of its own, as a clip's `mask` is written.
    mask: Option<Value>,
}

fn cover() -> Place {
    Place { fit: "cover", k: 1.0, ..Place::default() }
}

/// A slow push in, as photos in a reel breathe.
fn breathe(from: f64, to: f64) -> Vec<(&'static str, f64, f64)> {
    vec![("scale", 0.0, from), ("scale", 1.0, to)]
}

/// One template, as the app lists it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BuiltinTemplate {
    /// What the engine knows it by.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it does, in a line.
    pub line: String,
    /// How many photos or videos it takes.
    pub places: u32,
    /// The graphics it wants drawn, by name: "ring", "frame", "heart", or a white half "panel-…".
    pub graphics: Vec<String>,
}

fn entry(id: &str, name: &str, line: &str, places: u32, graphics: &[&str]) -> BuiltinTemplate {
    BuiltinTemplate {
        id: id.to_owned(),
        name: name.to_owned(),
        line: line.to_owned(),
        places,
        graphics: graphics.iter().map(|g| (*g).to_owned()).collect(),
    }
}

/// The built-in templates, in the order they are shown.
#[uniffi::export]
pub fn builtin_templates() -> Vec<BuiltinTemplate> {
    vec![
        entry("year-roll", "Year Roll", "The year rolls up to this one over your photo.", 1, &[]),
        entry("day-count", "Day Count", "A big glowing number behind you, the day of your series.", 1, &[]),
        entry("halo-circle", "Halo Circle", "A white ring behind you that pops in.", 1, &["ring"]),
        entry("halo-frame", "Halo Frame", "A white frame behind you.", 1, &["frame"]),
        entry("halo-heart", "Halo Heart", "A white heart behind you.", 1, &["heart"]),
        entry("depth-slide", "Depth Slide", "Your next photo slides in behind you.", 2, &[]),
        entry("split-reveal", "Split Reveal", "A second photo wipes in beside the first.", 2, &[]),
        entry("wide-roll", "Wide Roll", "Photos rolled past like one wide picture.", 4, &[]),
        entry("velocity", "Velocity", "Quick punchy cuts with flashes, on the beat.", 6, &[]),
        entry("flash-shake", "Flash & Shake", "Hard flashes and a shaken camera.", 3, &[]),
        entry("glow-rush", "Glow Rush", "A glowing, colour-split edit in an anime style.", 4, &[]),
        entry("seamless-zoom", "Seamless Zoom", "Each photo zooms into the next.", 4, &[]),
        entry("beat-flash", "Beat Flash", "Punch zooms and reversed moves, with white panels flashing on the beat.", 8, &["panel-right", "panel-left", "panel-bottom", "panel-top"]),
        entry("encore", "Encore", "Ramped zooms, a rewind, a mirror and an echo, for stage and street clips.", 3, &[]),
        entry("into-the-eye", "Into the Eye", "A deep zoom into a face, then the next shot.", 2, &[]),
    ]
}

/// A title of the template: words, where, when and how it looks.
struct Title {
    at: f64,
    length: f64,
    style: Value,
    x: f64,
    y: f64,
    keys: Vec<(&'static str, f64, f64)>,
    fx: Vec<Value>,
    /// Behind the person in the picture under it, once one is put there.
    behind: bool,
}

fn title(words: &str, at: f64, length: f64, y: f64, style: Value) -> Title {
    let mut style = style;
    style["content"] = json!(words);
    Title { at, length, style, x: 0.0, y, keys: Vec::new(), fx: Vec::new(), behind: false }
}

fn bold(size: f64, colour: &str) -> Value {
    json!({"fontFamily": "Bowlby One SC", "fontSize": size, "fontWeight": 400.0, "color": colour, "shadow": false})
}

fn sans(size: f64, colour: &str) -> Value {
    json!({"fontFamily": "Poppins", "fontSize": size, "fontWeight": 900.0, "color": colour, "shadow": false})
}

fn glow(colour: u32, strength: u32) -> Value {
    json!({"id": "capcraft.text-glow", "params": {"color": colour, "strength": strength, "range": 60}})
}

/// Rises into place as it comes in.
fn rise() -> Vec<(&'static str, f64, f64)> {
    vec![("opacity", 0.0, 0.0), ("opacity", 0.15, 1.0)]
}

/// A graphic of the template: which, when, how big, where, its keys, and
/// whether it goes behind the person.
struct Graphic {
    name: &'static str,
    at: f64,
    length: f64,
    scale: f64,
    x: f64,
    y: f64,
    keys: Vec<(&'static str, f64, f64)>,
    behind: bool,
}

fn graphic(name: &'static str, at: f64, length: f64, scale: f64, keys: Vec<(&'static str, f64, f64)>, behind: bool) -> Graphic {
    Graphic { name, at, length, scale, x: 0.0, y: -0.05, keys, behind }
}

/// A whole template: its places on the main track, places over them, titles,
/// graphics, short flashes and the joins between the main places.
struct Plan {
    main: Vec<(f64, Place, Option<(&'static str, f64)>)>,
    over: Vec<(f64, f64, Place, bool)>,
    titles: Vec<Title>,
    graphics: Vec<Graphic>,
    layers: Vec<(&'static str, f64, f64, Value)>,
}

fn plan(id: &str) -> Option<Plan> {
    let empty = Plan { main: Vec::new(), over: Vec::new(), titles: Vec::new(), graphics: Vec::new(), layers: Vec::new() };
    let one = |length: f64, from: f64, to: f64| (length, Place { keys: breathe(from, to), ..cover() }, None);
    let flash = |at: f64| ("capcraft.flash", at, 0.25, json!({"amount": 70, "speed": 2}));
    Some(match id {
        "year-roll" => {
            let mut titles = Vec::new();
            for (i, year) in ["2023", "2024", "2025"].iter().enumerate() {
                let mut t = title(year, 0.3 + i as f64 * 0.35, 0.35, -0.05, bold(0.11, "#ffffff"));
                t.keys = vec![("offsetY", 0.0, 0.05), ("offsetY", 0.45, -0.05), ("opacity", 0.0, 0.0), ("opacity", 0.3, 1.0), ("opacity", 0.85, 1.0), ("opacity", 1.0, 0.0)];
                titles.push(t);
            }
            let mut last = title("2026", 1.35, 3.15, -0.05, bold(0.13, "#ffffff"));
            last.keys = vec![("scale", 0.0, 1.6), ("scale", 0.12, 1.0), ("opacity", 0.0, 0.0), ("opacity", 0.06, 1.0)];
            last.fx = vec![glow(0xffffd36e, 70)];
            titles.push(last);
            titles.push(title("NEW YEAR, NEW CHAPTER", 1.8, 2.7, 0.08, sans(0.026, "#ffffff")));
            Plan { main: vec![one(4.5, 1.0, 1.1)], titles, layers: vec![flash(1.35)], ..empty }
        }
        "day-count" => {
            let mut number = title("03", 0.2, 4.3, -0.12, bold(0.3, "#ffd23f"));
            number.behind = true;
            number.fx = vec![glow(0xffffb703, 85)];
            number.keys = vec![("scale", 0.0, 1.3), ("scale", 0.12, 1.0), ("opacity", 0.0, 0.0), ("opacity", 0.08, 1.0)];
            let mut day = title("DAY", 0.2, 4.3, -0.3, sans(0.04, "#ffffff"));
            day.keys = rise();
            let mut series = title("EDITING SERIES", 0.5, 4.0, 0.26, bold(0.05, "#ffd23f"));
            series.keys = rise();
            Plan { main: vec![one(4.5, 1.0, 1.06)], titles: vec![number, day, series], layers: vec![flash(0.2)], ..empty }
        }
        "halo-circle" | "halo-frame" | "halo-heart" => {
            let name = match id {
                "halo-circle" => "ring",
                "halo-frame" => "frame",
                _ => "heart",
            };
            let pop = vec![("scale", 0.0, 0.2), ("scale", 0.1, 1.08), ("scale", 0.16, 1.0), ("opacity", 0.0, 0.0), ("opacity", 0.06, 1.0)];
            let mut words = title("main character", 0.6, 3.9, 0.3, json!({"fontFamily": "Instrument Serif", "fontSize": 0.05, "italic": true, "color": "#ffffff", "shadow": false}));
            words.keys = rise();
            Plan {
                main: vec![one(4.5, 1.0, 1.05)],
                graphics: vec![graphic(name, 0.2, 4.3, 0.9, pop, true)],
                titles: vec![words],
                ..empty
            }
        }
        "depth-slide" => {
            let slide = Place { keys: vec![("offsetX", 0.0, 1.0), ("offsetX", 0.35, 0.0)], ..cover() };
            let mut words = title("next stop", 1.4, 2.6, 0.32, json!({"fontFamily": "Instrument Serif", "fontSize": 0.06, "italic": true, "color": "#ffffff", "shadow": false}));
            words.keys = rise();
            Plan { main: vec![one(4.5, 1.0, 1.05)], over: vec![(1.0, 3.5, slide, true)], titles: vec![words], ..empty }
        }
        "split-reveal" => {
            let mut right = cover();
            right.fx = Vec::new();
            let mut words = title("BEFORE", 0.3, 4.2, 0.36, sans(0.035, "#ffffff"));
            words.x = -0.25;
            let mut after = title("AFTER", 1.2, 3.3, 0.36, sans(0.035, "#ffffff"));
            after.x = 0.25;
            after.keys = rise();
            Plan {
                main: vec![one(4.5, 1.0, 1.04)],
                over: vec![(0.8, 3.7, Place { keys: vec![("maskX", 0.0, 0.5), ("maskX", 0.25, 0.0)], ..right }, false)],
                titles: vec![words, after],
                ..empty
            }
        }
        "wide-roll" => {
            let pan = |len: f64| (len, Place { k: 1.35, keys: vec![("offsetX", 0.0, 0.15), ("offsetX", 1.0, -0.15)], ..cover() }, Some(("concat.push", 0.35)));
            let mut words = title("WIDE ANGLE", 0.3, 5.5, 0.34, bold(0.05, "#ffffff"));
            words.keys = rise();
            Plan { main: vec![pan(1.6), pan(1.6), pan(1.6), pan(1.6)], titles: vec![words], ..empty }
        }
        "velocity" => {
            let punch = |join| (0.6, Place { keys: vec![("scale", 0.0, 1.25), ("scale", 0.5, 1.0)], ..cover() }, join);
            Plan {
                main: vec![punch(None), punch(Some(("concat.whip-pan", 0.2))), punch(None), punch(Some(("concat.whip-pan", 0.2))), punch(None), punch(Some(("concat.zoom-blur-transition", 0.25)))],
                layers: (0..6).map(|i| flash(i as f64 * 0.6)).collect(),
                ..empty
            }
        }
        "flash-shake" => {
            let shaken = |join| (1.5, Place { fx: vec![json!({"id": "capcraft.shake", "params": {"amount": 30, "speed": 7}})], ..cover() }, join);
            Plan { main: vec![shaken(None), shaken(Some(("capcraft.fade-white", 0.2))), shaken(Some(("capcraft.fade-white", 0.2)))], layers: vec![flash(0.0), flash(1.5), flash(3.0)], ..empty }
        }
        "glow-rush" => {
            let glowing = |join| {
                (0.9, Place {
                    keys: vec![("scale", 0.0, 1.0), ("scale", 0.5, 1.12), ("scale", 1.0, 1.0)],
                    fx: vec![json!({"id": "concat.glow", "params": {"amount": 60}}), json!({"id": "concat.chromatic-aberration", "params": {"shift": 6}})],
                    ..cover()
                }, join)
            };
            Plan { main: vec![glowing(None), glowing(Some(("capcraft.flash-zoom", 0.25))), glowing(None), glowing(Some(("capcraft.flash-zoom", 0.25)))], layers: vec![flash(0.0), flash(1.8)], ..empty }
        }
        "seamless-zoom" => {
            let zoom = |join| (1.4, Place { keys: breathe(1.0, 1.18), ..cover() }, join);
            Plan { main: vec![zoom(None), zoom(Some(("concat.zoom-blur-transition", 0.4))), zoom(Some(("concat.zoom-blur-transition", 0.4))), zoom(Some(("concat.zoom-blur-transition", 0.4)))], ..empty }
        }
        "into-the-eye" => {
            let deep = Place { keys: vec![("scale", 0.0, 1.0), ("scale", 1.0, 3.2), ("offsetY", 0.0, 0.0), ("offsetY", 1.0, 0.45)], fx: vec![json!({"id": "concat.glow", "params": {"amount": 35}})], ..cover() };
            Plan { main: vec![(2.6, deep, None), (1.8, Place { keys: breathe(1.2, 1.0), ..cover() }, Some(("capcraft.flash-zoom", 0.3)))], layers: vec![flash(2.5)], ..empty }
        }
        "beat-flash" => {
            // a shot a bar: punched out and settling, then pushed in with its video backwards
            let shot = |i: usize| {
                let back = i % 2 == 1;
                let keys = if back { breathe(1.0, 1.26) } else { vec![("scale", 0.0, 1.45), ("scale", 0.2, 1.02), ("scale", 1.0, 1.0)] };
                (0.9, Place { keys, reverse: back, ..cover() }, None)
            };
            // and on each beat in it a white panel over one half of the frame, round the halves
            let halves = ["panel-right", "panel-left", "panel-bottom", "panel-top"];
            let panels = (0..16)
                .map(|n| {
                    let at = (n / 2) as f64 * 0.9 + if n % 2 == 0 { 0.3 } else { 0.6 };
                    Graphic { y: 0.0, ..graphic(halves[n % 4], at, 0.2, 1.0, vec![("opacity", 0.0, 1.0), ("opacity", 1.0, 0.6)], false) }
                })
                .collect();
            Plan { main: (0..8).map(shot).collect(), graphics: panels, ..empty }
        }
        "encore" => {
            // a clip ramped and pushed in, then rewound; the next in mono, then mirrored; the last with an echo over it
            let zoom = |from: f64, to: f64| vec![("scale", 0.0, from), ("scale", 1.0, to)];
            let mono = json!({"id": "concat.saturation", "params": {"amount": -100}});
            let glow = json!({"id": "concat.glow", "params": {"amount": 40}});
            let main = vec![
                (1.3, Place { keys: zoom(1.0, 1.3), ramp: true, ..cover() }, None),
                (0.9, Place { keys: zoom(1.3, 1.0), reverse: true, copy_of: Some(0), ..cover() }, None),
                (1.2, Place { keys: zoom(1.0, 1.25), ramp: true, fx: vec![mono], ..cover() }, None),
                (1.0, Place { keys: zoom(1.25, 1.05), flip: true, copy_of: Some(2), ..cover() }, Some(("capcraft.flash-zoom", 0.25))),
                (1.8, Place { keys: zoom(1.0, 1.3), ramp: true, fx: vec![glow], ..cover() }, None),
            ];
            let echo = Place { k: 1.18, keys: vec![("scale", 0.0, 1.0), ("scale", 1.0, 1.3), ("opacity", 0.0, 0.4), ("opacity", 1.0, 0.4)], copy_of: Some(4), blend: "screen", ..cover() };
            let white = |at: f64| ("capcraft.flash", at, 0.25, json!({"amount": 85, "speed": 2}));
            Plan { main, over: vec![(4.4, 1.8, echo, false)], layers: vec![white(2.15), white(4.35)], ..empty }
        }
        _ => return None,
    })
}

fn key(clip: &str, property: &str, at: f64, value: f64) -> Value {
    json!({"op": "setClipKey", "clipId": clip, "property": property, "at": at.clamp(0.0, 1.0), "value": value, "ease": if property.starts_with("mask") { EVEN } else { SETTLE }})
}

fn place_json(place: &Place) -> Value {
    json!({
        "base": {"fit": place.fit, "k": place.k, "x": place.x, "y": place.y, "rotation": place.rotation},
        "keys": place.keys.iter().map(|(p, at, v)| json!([p, at, v])).collect::<Vec<_>>(),
        "fx": place.fx,
        "reverse": place.reverse,
        "flip": place.flip,
        "ramp": place.ramp,
        "blend": place.blend,
        "filled": false,
        "edited": true,
    })
}

#[uniffi::export]
impl MobileSession {
    /// Lays built-in template `id` out on this (empty) project: `files` names
    /// the picture for each place ("p1", "p2" and on) and each graphic it
    /// wants ([`BuiltinTemplate::graphics`]).
    pub fn build_template(&self, id: String, files: HashMap<String, String>) -> Result<TimelineView, MobileError> {
        let plan = plan(&id).ok_or_else(|| MobileError::Failed("There is no such template.".to_owned()))?;
        let file = |name: &str| files.get(name).cloned().ok_or_else(|| MobileError::Failed(format!("The template has no {name}.")));
        let mut slots = serde_json::Map::new();
        let mut behind = Vec::new();
        let mut places: Vec<(String, String)> = Vec::new();

        // the user's places are numbered in turn; a copy takes its original's picture
        let mut names: Vec<String> = Vec::new();
        for place in plan.main.iter().map(|(_, p, _)| p).chain(plan.over.iter().map(|(_, _, p, _)| p)) {
            let name = match place.copy_of {
                Some(k) => names.get(k).cloned().unwrap_or_else(|| "p1".to_owned()),
                None => format!("p{}", names.iter().collect::<std::collections::HashSet<_>>().len() + 1),
            };
            names.push(name);
        }

        // the places on the main track, end to end
        let mut ids = Vec::new();
        for name in names.iter().take(plan.main.len()) {
            ids.push(self.import(file(name)?)?.id);
        }
        let view = self.add_to_main(ids, 0)?;
        let mut main: Vec<_> = view
            .rows
            .iter()
            .find(|row| row.kind == crate::lanes::RowKind::Main)
            .map(|row| row.clips.clone())
            .unwrap_or_default();
        main.sort_by_key(|clip| clip.start_us);
        let mut commands = Vec::new();
        for (clip, (length, _, _)) in main.iter().zip(&plan.main) {
            commands.push(json!({"op": "trimClip", "clipId": clip.id, "edge": "end", "delta": length - clip.duration_us as f64 / 1e6, "ripple": true}));
        }
        self.run(commands.iter().filter_map(|c| serde_json::from_value(c.clone()).ok()).collect())?;
        let mut commands = Vec::new();
        for (i, (clip, (_, place, join))) in main.iter().zip(&plan.main).enumerate() {
            if let Some((transition, seconds)) = join
                && i > 0
            {
                commands.push(json!({"op": "updateClip", "clipId": clip.id, "patch": {"transitionIn": {"id": transition, "duration": seconds}}}));
            }
            let mut slot = place_json(place);
            match place.copy_of.and_then(|k| main.get(k)) {
                Some(original) => slot["copyOf"] = json!(original.id),
                None => places.push((clip.id.clone(), file(&names[i])?)),
            }
            slots.insert(clip.id.clone(), slot);
        }

        // places over them; a copy may copy any place before it
        let mut made_ids: Vec<String> = main.iter().map(|c| c.id.clone()).collect();
        for (n, (at, length, place, person_over)) in plan.over.iter().enumerate() {
            let name = names[plan.main.len() + n].clone();
            let media = self.import(file(&name)?)?;
            let made = self.add_overlay(media.id, micros(*at))?.created_id;
            made_ids.push(made.clone());
            if made.is_empty() {
                continue;
            }
            let now = self.view().rows.iter().flat_map(|r| r.clips.clone()).find(|c| c.id == made).map_or(*length, |c| c.duration_us as f64 / 1e6);
            commands.push(json!({"op": "trimClip", "clipId": made, "edge": "end", "delta": length - now, "ripple": false}));
            if place.keys.iter().any(|(p, _, _)| p.starts_with("mask")) {
                commands.push(json!({"op": "updateClip", "clipId": made, "patch": {"mask": {"shape": "linear", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0, "rotation": 90.0, "roundness": 0.0, "feather": 0.01, "invert": false}}}));
                for (p, at, v) in &place.keys {
                    commands.push(key(&made, p, *at, *v));
                }
            }
            if let Some(mask) = &place.mask {
                commands.push(json!({"op": "updateClip", "clipId": made, "patch": {"mask": mask}}));
            }
            if *person_over {
                behind.push(json!(made));
            }
            let mut slot = place_json(place);
            match place.copy_of.and_then(|k| made_ids.get(k)).filter(|id| !id.is_empty()) {
                Some(original) => slot["copyOf"] = json!(original),
                None => places.push((made.clone(), file(&name)?)),
            }
            slots.insert(made, slot);
        }

        // titles, each with its look, keys and place
        for t in &plan.titles {
            let made = self.add_text(micros(t.at), micros(t.length), t.style.to_string())?.created_id;
            if made.is_empty() {
                continue;
            }
            commands.push(json!({"op": "setClipTransform", "clipId": made, "offsetX": t.x, "offsetY": t.y}));
            for (p, at, v) in &t.keys {
                commands.push(key(&made, p, *at, *v));
            }
            if !t.fx.is_empty() {
                commands.push(json!({"op": "updateClip", "clipId": made, "patch": {"videoEffects": t.fx}}));
            }
            if t.behind {
                behind.push(json!(made));
            }
        }

        // graphics
        let mut imported: HashMap<&str, String> = HashMap::new();
        for g in &plan.graphics {
            let media = match imported.get(g.name) {
                Some(id) => id.clone(),
                None => {
                    let id = self.import(file(g.name)?)?.id;
                    imported.insert(g.name, id.clone());
                    id
                }
            };
            let made = self.add_sticker(media, micros(g.at), micros(g.length))?.created_id;
            if made.is_empty() {
                continue;
            }
            commands.push(json!({"op": "setClipTransform", "clipId": made, "scale": g.scale, "offsetX": g.x, "offsetY": g.y}));
            for (p, at, v) in &g.keys {
                commands.push(key(&made, p, *at, if *p == "scale" { v * g.scale } else { *v }));
            }
            if g.behind {
                behind.push(json!(made));
            }
        }

        // short flashes and the like, on rows of their own
        for (effect, at, length, params) in &plan.layers {
            let made = self.add_layer((*effect).to_owned(), "Flash".to_owned(), micros(*at), micros(*length))?.created_id;
            if !made.is_empty() {
                commands.push(json!({"op": "updateClip", "clipId": made, "patch": {"videoEffects": [{"id": effect, "params": params}]}}));
            }
        }
        // its music, cut to its length
        if let Some(music) = files.get("music") {
            let length: f64 = plan.main.iter().map(|(l, _, _)| l).sum();
            let media = self.import(music.clone())?;
            let made = self.add_audio(media.id, 0)?.created_id;
            if !made.is_empty() {
                let now = self.view().rows.iter().flat_map(|r| r.clips.clone()).find(|c| c.id == made).map_or(length, |c| c.duration_us as f64 / 1e6);
                commands.push(json!({"op": "trimClip", "clipId": made, "edge": "end", "delta": length - now, "ripple": false}));
            }
        }
        commands.push(json!({"op": "setTimelineExtra", "key": REEL, "value": {"slots": slots, "behind": behind, "worded": true, "template": id}}));
        self.run(commands.into_iter().filter_map(|c| serde_json::from_value(c).ok()).collect())?;

        // the placeholders put in as a picture would be, so the template shows as it will move
        for (clip, path) in places {
            self.fill_slot(clip, path, true, false)?;
        }
        let mut kept: Value = serde_json::from_str(&self.extra(REEL.to_owned())).unwrap_or_else(|_| json!({}));
        if let Some(slots) = kept.get_mut("slots").and_then(Value::as_object_mut) {
            for slot in slots.values_mut() {
                slot["filled"] = json!(false);
            }
        }
        let command = serde_json::from_value(json!({"op": "setTimelineExtra", "key": REEL, "value": kept}))
            .map_err(|error| MobileError::Failed(error.to_string()))?;
        self.run(vec![command])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_template_has_a_plan_with_its_places() {
        for t in builtin_templates() {
            let plan = plan(&t.id).unwrap_or_else(|| panic!("{}", t.id));
            let own = plan.main.iter().map(|(_, p, _)| p).chain(plan.over.iter().map(|(_, _, p, _)| p)).filter(|p| p.copy_of.is_none()).count();
            assert_eq!(own as u32, t.places, "{}", t.id);
            for g in &plan.graphics {
                assert!(t.graphics.iter().any(|name| name == g.name), "{}", t.id);
            }
        }
        let _ = HashMap::<String, String>::new();
    }
}
