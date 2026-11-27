// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The user's own voice, for reading words aloud: a recording of them
//! saying, in their own words' sense, that it is theirs and may be copied.
//!
//! Only that recording is taken as a voice. It is heard here and checked
//! against the sentence it was asked to say, so a sound of somebody else -
//! a file, a call, a video - is not one. It stays on the phone, and goes
//! when the user says so.

use std::path::PathBuf;
use std::sync::Arc;

use concat_media::{AudioDecoder, AudioOptions, SampleFormat};
use concat_speech::transcribe::TranscribeRequest;
use concat_speech::tts::{POCKET_CLONE, Reference};

use crate::speech::{SpeechListener, stage_of, transcriber};
use crate::{MobileError, context};

/// What the user says, and agrees to by saying.
const SENTENCE: &str = "This is my own voice, and I agree that this app may make a copy of it on this phone, to read my words aloud.";

/// How much of the sentence has to be heard, of its words.
const HEARD: f64 = 0.7;

fn kept() -> Result<PathBuf, MobileError> {
    Ok(context()?.dirs.data.join("my-voice.wav"))
}

/// The voice a reading takes: the user's own recording where `voice` is
/// theirs, none otherwise.
pub(crate) fn reference(voice: i32) -> Result<Option<Reference>, MobileError> {
    if voice != POCKET_CLONE {
        return Ok(None);
    }
    let path = kept()?;
    if !path.is_file() {
        return Err(MobileError::Failed("Record your voice first.".to_owned()));
    }
    Ok(Some(Reference {
        path: path.to_string_lossy().into_owned(),
        start: 0.0,
    }))
}

/// The words of `text`, bare.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

/// How much of `asked` is in `said`, in order, 0 to 1.
fn agreement(asked: &str, said: &str) -> f64 {
    let (asked, said) = (words(asked), words(said));
    if asked.is_empty() {
        return 0.0;
    }
    // the longest run of the one in the other, skips allowed
    let mut row = vec![0usize; said.len() + 1];
    for a in &asked {
        let mut before = 0;
        for (j, s) in said.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = if a == s {
                before + 1
            } else {
                here.max(row[j])
            };
            before = here;
        }
    }
    row[said.len()] as f64 / asked.len() as f64
}

/// What the user reads out to have their voice copied.
#[uniffi::export]
pub fn my_voice_sentence() -> String {
    SENTENCE.to_owned()
}

/// The voice's number, for the calls that read aloud.
#[uniffi::export]
pub fn my_voice_id() -> i32 {
    POCKET_CLONE
}

/// Whether the user's voice is kept.
#[uniffi::export]
pub fn has_my_voice() -> bool {
    kept().is_ok_and(|path| path.is_file())
}

/// Forgets the user's voice: the recording is deleted.
#[uniffi::export]
pub fn forget_my_voice() -> Result<(), MobileError> {
    let path = kept()?;
    if path.is_file() {
        std::fs::remove_file(&path)
            .map_err(|error| MobileError::Failed(format!("could not delete it: {error}")))?;
    }
    Ok(())
}

/// Checks `recording`, heard with `model_id`, against the sentence, and
/// keeps it as the user's voice. Runs on a thread of its own; `on_done`
/// carries nothing.
#[uniffi::export]
pub fn keep_my_voice(
    recording: String,
    model_id: String,
    listener: Arc<dyn SpeechListener>,
) -> Result<(), MobileError> {
    let dirs = context()?.dirs.clone();
    let to = kept()?;
    std::thread::Builder::new()
        .name("capcraft-my-voice".to_owned())
        .spawn(move || {
            let progress = Arc::clone(&listener);
            let heard = transcriber().transcribe(
                &dirs,
                &TranscribeRequest {
                    path: recording.clone(),
                    audio_stream: None,
                    source_start: 0.0,
                    window: 60.0,
                    model_id,
                    words: false,
                    translate: false,
                    // whisper hears which language it is
                    language: None,
                },
                move |stage| {
                    let (done, name) = stage_of(stage);
                    progress.on_progress(done, name);
                },
            );
            let said = match heard {
                Ok(segments) => segments
                    .into_iter()
                    .map(|segment| segment.text)
                    .collect::<Vec<_>>()
                    .join(" "),
                Err(message) => {
                    listener.on_failed(message);
                    return;
                }
            };
            if agreement(SENTENCE, &said) < HEARD {
                listener.on_failed(
                    "That was not the sentence. Read it out as it is written, somewhere quiet."
                        .to_owned(),
                );
                return;
            }
            match write(&recording, &to) {
                Ok(()) => listener.on_done(None),
                Err(message) => listener.on_failed(message),
            }
        })
        .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
    Ok(())
}

/// The recording as a mono WAV at `to`, its quiet start left out.
fn write(recording: &str, to: &std::path::Path) -> Result<(), String> {
    const RATE: u32 = 24_000;
    let samples = AudioDecoder::open(
        recording,
        &AudioOptions {
            start: None,
            duration: None,
            filters: Vec::new(),
            rate: RATE,
            channels: 1,
            format: SampleFormat::F32,
            stream: None,
            stretch: None,
        },
    )
    .map_err(|error| error.to_string())?
    .collect_f32()
    .map_err(|error| error.to_string())?;
    let first = samples
        .iter()
        .position(|sample| sample.abs() > 0.02)
        .unwrap_or(0)
        .saturating_sub(RATE as usize / 10);
    let voice = &samples[first..];
    let bytes = voice.len() as u32 * 2;
    let mut wav = Vec::with_capacity(44 + bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    for sample in voice {
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(to, wav).map_err(|error| format!("could not keep the voice: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentence_heard_almost_right_agrees() {
        let said = "this is my own voice and I agree that the app may make a copy of it on this phone to read my words out loud";
        assert!(agreement(SENTENCE, said) >= HEARD);
    }

    #[test]
    fn something_else_said_does_not_agree() {
        let said = "hello everyone and welcome back to the channel today we will talk about my own phone";
        assert!(agreement(SENTENCE, said) < 0.5);
        assert_eq!(agreement(SENTENCE, ""), 0.0);
    }
}
