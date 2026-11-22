// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Groups: clips off the main track that go together. One moved along the
//! timeline takes the others by as much, and one deleted takes them all.
//! The timeline keeps which clips are together under `capcraftGroups`, a
//! list of lists of ids; a clip is in one group at most, and an id no clip
//! has any more is passed over.

use concat_project::Command;
use concat_project::commands::ClipMove;
use concat_project::model::Timeline;
use serde_json::{Value, json};

use crate::MobileError;
use crate::lanes;
use crate::session::{MobileSession, TimelineView};

/// Where the timeline keeps its groups.
pub const GROUPS: &str = "capcraftGroups";

/// The groups of `timeline`, each two clips or more still on it.
pub(crate) fn groups(timeline: &Timeline) -> Vec<Vec<String>> {
    let Some(Value::Array(all)) = timeline.extra.get(GROUPS) else {
        return Vec::new();
    };
    all.iter()
        .filter_map(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .filter(|id| timeline.clips.iter().any(|clip| clip.id == *id))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|ids| ids.len() > 1)
        .collect()
}

/// `ids` and every clip grouped with one of them.
pub(crate) fn with_groups(timeline: &Timeline, ids: Vec<String>) -> Vec<String> {
    let mut out = ids;
    for group in groups(timeline) {
        if group.iter().any(|id| out.contains(id)) {
            for id in group {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// The moves that take the clips grouped with `moved` by `shift` seconds, on
/// their own lanes where there is room and on a free lane of their kind
/// where there is not. None of them goes before the start: the group stops
/// as one.
pub(crate) fn along(timeline: &Timeline, moved: &str, shift: f64) -> Vec<ClipMove> {
    let Some(group) = groups(timeline)
        .into_iter()
        .find(|ids| ids.iter().any(|id| id == moved))
    else {
        return Vec::new();
    };
    let moving: Vec<&str> = group.iter().map(String::as_str).collect();
    timeline
        .clips
        .iter()
        .filter(|clip| clip.id != moved && group.contains(&clip.id))
        .map(|clip| {
            let start = (clip.start + shift).max(0.0);
            let track = if timeline.is_free(&clip.track_id, start, clip.duration, &moving) {
                clip.track_id.clone()
            } else {
                lanes::free_lane(timeline, lanes::kind_of(clip), start, clip.duration)
                    .unwrap_or_else(|| clip.track_id.clone())
            };
            ClipMove {
                clip_id: clip.id.clone(),
                track_id: track,
                start,
            }
        })
        .collect()
}

/// The earliest start among the clips grouped with `id`, itself included.
pub(crate) fn earliest(timeline: &Timeline, id: &str) -> Option<f64> {
    let group = groups(timeline)
        .into_iter()
        .find(|ids| ids.iter().any(|held| held == id))?;
    timeline
        .clips
        .iter()
        .filter(|clip| group.contains(&clip.id))
        .map(|clip| clip.start)
        .min_by(f64::total_cmp)
}

#[uniffi::export]
impl MobileSession {
    /// Groups clips off the main track, out of whatever groups they were in.
    pub fn group(&self, clip_ids: Vec<String>) -> Result<TimelineView, MobileError> {
        let (next, count) = self.with_session(|session| {
            let timeline = session.project().active();
            let ids: Vec<String> = clip_ids
                .iter()
                .filter(|id| {
                    !lanes::on_main(timeline, id) && timeline.clips.iter().any(|c| c.id == **id)
                })
                .cloned()
                .collect();
            let mut all: Vec<Vec<String>> = groups(timeline)
                .into_iter()
                .map(|group| group.into_iter().filter(|id| !ids.contains(id)).collect())
                .filter(|group: &Vec<String>| group.len() > 1)
                .collect();
            let count = ids.len();
            all.push(ids);
            Ok((all, count))
        })?;
        if count < 2 {
            return Err(MobileError::Failed(
                "Choose two or more clips off the main track to group.".to_owned(),
            ));
        }
        self.run(vec![Command::SetTimelineExtra {
            key: GROUPS.to_owned(),
            value: Some(json!(next)),
        }])
    }

    /// Takes the group `clip_id` is in apart.
    pub fn ungroup(&self, clip_id: String) -> Result<TimelineView, MobileError> {
        let left: Vec<Vec<String>> = self.with_session(|session| {
            Ok(groups(session.project().active())
                .into_iter()
                .filter(|group| !group.contains(&clip_id))
                .collect())
        })?;
        self.run(vec![Command::SetTimelineExtra {
            key: GROUPS.to_owned(),
            value: (!left.is_empty()).then(|| json!(left)),
        }])
    }

    /// The clips grouped with `clip_id`, itself included; none when it is in
    /// no group.
    pub fn group_of(&self, clip_id: String) -> Vec<String> {
        self.with_session(|session| {
            Ok(groups(session.project().active())
                .into_iter()
                .find(|group| group.contains(&clip_id))
                .unwrap_or_default())
        })
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concat_project::model::{Clip, ClipKind, Track};
    use std::sync::Arc;

    fn timeline(groups: Value) -> Timeline {
        let mut timeline = Timeline::default();
        for id in ["main", "a", "b"] {
            timeline.tracks.push(Track {
                id: id.to_owned(),
                visible: true,
                muted: false,
                extra: Default::default(),
            });
        }
        timeline.clips = [("1", "a", 1.0), ("2", "b", 2.0), ("3", "b", 8.0)]
            .into_iter()
            .map(|(id, track, start)| {
                Arc::new(Clip {
                    id: id.to_owned(),
                    track_id: track.to_owned(),
                    kind: ClipKind::Text,
                    start,
                    duration: 2.0,
                    ..Clip::default()
                })
            })
            .collect();
        timeline.extra.insert(GROUPS.to_owned(), groups);
        timeline
    }

    #[test]
    fn the_others_of_a_group_go_as_far() {
        let t = timeline(json!([["1", "2"]]));
        let moves = along(&t, "1", 1.5);
        assert_eq!(moves.len(), 1);
        assert_eq!((moves[0].clip_id.as_str(), moves[0].start), ("2", 3.5));
        assert_eq!(moves[0].track_id, "b");
        assert!(along(&t, "3", 1.0).is_empty());
    }

    #[test]
    fn a_group_is_taken_whole_and_a_gone_clip_is_passed_over() {
        let t = timeline(json!([["1", "2", "gone"], ["3", "gone"]]));
        assert_eq!(with_groups(&t, vec!["2".to_owned()]), vec!["2", "1"]);
        // one clip left is no group
        assert_eq!(groups(&t).len(), 1);
        assert_eq!(earliest(&t, "2"), Some(1.0));
    }
}
