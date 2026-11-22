// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Filler words: the "um"s and "uh"s between what is said, found by their
//! times and cut out of the main track.
//!
//! The talk is heard again with every word timed and hesitations written
//! down (see `TranscribeRequest::words`). What is found is offered, not
//! taken: the list comes back for the user to choose from, and a choice is
//! cut as pauses are, in one undo step.

use std::sync::Arc;

use concat_speech::transcribe::Word;

use crate::pauses::moved;
use crate::session::{MobileSession, TimelineView};
use crate::speech::{TranscriptLine, heard, stage_of, transcriber};
use crate::{MobileError, context, micros, seconds};

/// A filler found on the timeline.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Filler {
    /// Where it starts, microseconds into the timeline.
    pub start_us: i64,
    /// Where it ends.
    pub end_us: i64,
    /// What was said.
    pub text: String,
    /// Whether it is a sound that means nothing ("um"), safe to take out;
    /// otherwise a phrase that may mean something ("you know").
    pub sound: bool,
}

/// What a search for fillers reports.
#[uniffi::export(with_foreign)]
pub trait FillerListener: Send + Sync {
    /// How far along it is, 0 to 1, and what it is doing.
    fn on_progress(&self, fraction: f32, stage: String);
    /// It finished with these, in time order.
    fn on_done(&self, fillers: Vec<Filler>);
    /// It failed or was cancelled.
    fn on_failed(&self, message: String);
}

/// Sounds that mean nothing, as they are written down.
const SOUNDS: [&str; 20] = [
    "ah", "ahh", "ahhh",
    "um", "umm", "ummm", "uh", "uhh", "uhm", "uhhh", "er", "err", "erm", "ehm", "em", "hm", "hmm",
    "hmmm", "mm", "mmm",
];

/// Phrases said as fillers, a word each; they may be meant too.
const PHRASES: [&[&str]; 7] = [
    &["you", "know"],
    &["i", "mean"],
    &["sort", "of"],
    &["kind", "of"],
    &["like"],
    &["basically"],
    &["actually"],
];

/// The word without its punctuation and capitals; a drawn-out "ummmm" as
/// "ummm".
fn bare(word: &str) -> String {
    let mut out = String::new();
    for c in word.chars().filter(|c| c.is_alphanumeric()) {
        let c = c.to_lowercase().next().unwrap_or(c);
        if out.len() >= 3 && out.ends_with(&c.to_string().repeat(3)) {
            continue;
        }
        out.push(c);
    }
    out
}

/// The fillers among `words`: `(start, end, text, sound)` each, in their
/// time. Fillers with nothing between them are one.
fn find(words: &[Word]) -> Vec<(f64, f64, String, bool)> {
    let bares: Vec<String> = words.iter().map(|word| bare(&word.text)).collect();
    let mut found: Vec<(f64, f64, String, bool)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let sound = SOUNDS.contains(&bares[i].as_str());
        let phrase = PHRASES.iter().find(|phrase| {
            bares.len() >= i + phrase.len()
                && phrase.iter().zip(&bares[i..]).all(|(want, is)| want == is)
                // a word alone is a filler only where it is set off by a comma
                && (phrase.len() > 1 || words[i].text.ends_with(','))
        });
        let length = match (sound, phrase) {
            (true, _) => 1,
            (false, Some(phrase)) => phrase.len(),
            (false, None) => {
                i += 1;
                continue;
            }
        };
        let (start, end) = (words[i].start, words[i + length - 1].end);
        let text = words[i..i + length]
            .iter()
            .map(|word| word.text.trim_matches(|c: char| !c.is_alphanumeric()))
            .collect::<Vec<_>>()
            .join(" ");
        match found.last_mut() {
            Some(last) if last.3 == sound && start - last.1 < 0.05 && sound => {
                last.1 = end.max(last.1);
                last.2 = format!("{} {text}", last.2);
            }
            _ => found.push((start, end, text, sound)),
        }
        i += length;
    }
    found.retain(|(start, end, ..)| end - start > 0.04);
    found
}

/// A stretch of the timeline.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TimeRange {
    /// Where it starts, microseconds.
    pub start_us: i64,
    /// Where it ends.
    pub end_us: i64,
}

/// What cutting stretches leaves: the timeline, and the transcript moved to
/// match it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RangesCut {
    /// The timeline without them.
    pub view: TimelineView,
    /// The transcript's lines where they now sit.
    pub lines: Vec<TranscriptLine>,
}

#[uniffi::export]
impl MobileSession {
    /// Finds the filler words in what is said on the main track, heard with
    /// `model_id`. Nothing is changed. Runs on a thread of its own.
    pub fn find_fillers(
        self: Arc<Self>,
        model_id: String,
        listener: Arc<dyn FillerListener>,
    ) -> Result<(), MobileError> {
        let dirs = context()?.dirs.clone();
        let windows = heard(&self, &model_id, true)?;
        if windows.is_empty() {
            return Err(MobileError::Failed(
                "There is no sound on the timeline to listen to.".to_owned(),
            ));
        }
        std::thread::Builder::new()
            .name("capcraft-fillers".to_owned())
            .spawn(move || {
                let count = windows.len() as f32;
                let mut fillers = Vec::new();
                for (index, (start, speed, request)) in windows.iter().enumerate() {
                    let progress = Arc::clone(&listener);
                    let base = index as f32;
                    let heard = transcriber().transcribe(&dirs, request, move |stage| {
                        let (done, name) = stage_of(stage);
                        progress.on_progress((base + done) / count, name);
                    });
                    let segments = match heard {
                        Ok(segments) => segments,
                        Err(message) => {
                            listener.on_failed(message);
                            return;
                        }
                    };
                    let words: Vec<Word> = segments.into_iter().flat_map(|s| s.words).collect();
                    let speed = speed.max(1e-6);
                    fillers.extend(find(&words).into_iter().map(|(from, to, text, sound)| Filler {
                        start_us: micros(start + from / speed),
                        end_us: micros(start + to / speed),
                        text,
                        sound,
                    }));
                }
                fillers.sort_by_key(|filler| filler.start_us);
                listener.on_done(fillers);
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }

    /// Cuts `ranges` out of the main track as one undo step, and moves
    /// `lines` (the talk's transcript) to match.
    pub fn cut_many(
        &self,
        ranges: Vec<TimeRange>,
        lines: Vec<TranscriptLine>,
    ) -> Result<RangesCut, MobileError> {
        let mut cuts: Vec<(f64, f64)> = ranges
            .iter()
            .map(|range| (seconds(range.start_us), seconds(range.end_us)))
            .filter(|(a, b)| b > a)
            .collect();
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        // overlapping stretches are one: each is cut where the ones before it were not
        let mut joined: Vec<(f64, f64)> = Vec::new();
        for (a, b) in cuts {
            match joined.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(b),
                _ => joined.push((a, b)),
            }
        }
        let view = self.cut_ranges(&joined)?;
        Ok(RangesCut {
            view,
            lines: moved(lines, &joined),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[(f64, f64, &str)]) -> Vec<Word> {
        list.iter()
            .map(|(start, end, text)| Word {
                start: *start,
                end: *end,
                text: (*text).to_owned(),
            })
            .collect()
    }

    #[test]
    fn hesitations_are_found_whatever_their_spelling() {
        let found = find(&words(&[
            (0.0, 0.3, "Ummmmm,"),
            (0.5, 0.8, "hello"),
            (1.0, 1.2, "Uh"),
            (1.3, 1.6, "there"),
        ]));
        let texts: Vec<&str> = found.iter().map(|f| f.2.as_str()).collect();
        assert_eq!(texts, vec!["Ummmmm", "Uh"]);
        assert!(found.iter().all(|f| f.3));
    }

    #[test]
    fn hesitations_back_to_back_are_one() {
        let found = find(&words(&[(0.0, 0.3, "um,"), (0.3, 0.6, "uh,"), (0.6, 1.0, "yes")]));
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0, found[0].1), (0.0, 0.6));
    }

    #[test]
    fn phrases_are_told_from_sounds_and_a_meant_like_is_left() {
        let found = find(&words(&[
            (0.0, 0.2, "I"),
            (0.2, 0.4, "like"),
            (0.4, 0.6, "it,"),
            (0.6, 0.8, "you"),
            (0.8, 1.0, "know,"),
            (1.0, 1.2, "like,"),
            (1.2, 1.4, "lots"),
        ]));
        let read: Vec<(&str, bool)> = found.iter().map(|f| (f.2.as_str(), f.3)).collect();
        assert_eq!(read, vec![("you know", false), ("like", false)]);
    }

    #[test]
    fn a_word_that_only_starts_like_a_filler_is_a_word() {
        assert!(find(&words(&[(0.0, 0.4, "umbrella"), (0.5, 0.9, "error")])).is_empty());
    }
}
