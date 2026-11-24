// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Sound: the engine's player, one per process, started on first use. It
//! keeps the clock the editor follows while playing: the app asks for the
//! position each frame and draws the picture for it.
//!
//! Whatever the open session makes audible is handed to the player after
//! every edit ([`sync`]), so play always sounds like the timeline.

use std::path::PathBuf;
use std::sync::Arc;

use concat_host::playback::{ClipSpec, GainKey, Playback, PlaybackEvents};
use concat_host::session::Session;

use crate::{MobileError, context, micros, seconds};

/// What the player reports that the app should hear about.
#[uniffi::export(with_foreign)]
pub trait PlaybackListener: Send + Sync {
    /// A failure the user should see (the sound device went away, a file
    /// would not decode).
    fn on_error(&self, message: String);
}

struct Events {
    listener: std::sync::Mutex<Option<Arc<dyn PlaybackListener>>>,
}

impl PlaybackEvents for Events {
    fn position(&self, _seconds: f64) {}

    fn error(&self, message: String) {
        log::warn!("playback: {message}");
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(listener) = listener {
            listener.on_error(message);
        }
    }
}

static EVENTS: std::sync::OnceLock<Arc<Events>> = std::sync::OnceLock::new();

fn events() -> Arc<Events> {
    Arc::clone(EVENTS.get_or_init(|| {
        Arc::new(Events {
            listener: std::sync::Mutex::new(None),
        })
    }))
}

/// Set once the sound device has failed to open: edits then stop trying,
/// and play says why.
static FAILED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The player, started if it was not. A device that will not open (or a
/// panic inside it) is an error, never a crash, and is not retried on every
/// edit.
fn player() -> Result<Arc<Playback>, MobileError> {
    let context = context()?;
    let mut slot = context
        .playback
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(player) = slot.as_ref() {
        return Ok(Arc::clone(player));
    }
    if let Some(reason) = FAILED.get() {
        return Err(MobileError::Failed(reason.clone()));
    }
    let events: Arc<dyn PlaybackEvents> = events();
    let started =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Playback::start(events)));
    let player = match started {
        Ok(Ok(player)) => player,
        Ok(Err(message)) => {
            let reason = format!("Sound is not available: {message}");
            let _ = FAILED.set(reason.clone());
            return Err(MobileError::Failed(reason));
        }
        Err(_) => {
            let reason = "Sound is not available on this device.".to_owned();
            let _ = FAILED.set(reason.clone());
            return Err(MobileError::Failed(reason));
        }
    };
    *slot = Some(Arc::clone(&player));
    Ok(player)
}

/// The player when it has been started, without starting it.
fn running() -> Option<Arc<Playback>> {
    let context = context().ok()?;
    context
        .playback
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Hands the session's audible clips to the player, when it is running.
pub(crate) fn sync(session: &Session) {
    let player = match running() {
        Some(player) => player,
        None => match player() {
            Ok(player) => player,
            Err(_) => return,
        },
    };
    let specs = specs(session);
    let path = PathBuf::from(session.path());
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        player.set_clips(path, specs)
    }));
}

/// The clips a session makes audible, as the player takes them: through the
/// exporter's own cut into pieces, so a speed curve sounds as it will in
/// the file.
fn specs(session: &Session) -> Vec<ClipSpec> {
    session
        .flattened_clips()
        .iter()
        .filter(|clip| {
            !clip.muted
                && (clip.kind == concat_export::ClipKind::Audio
                    || (clip.kind == concat_export::ClipKind::Video
                        && clip.has_audio.unwrap_or(true)))
        })
        .flat_map(concat_export::audio_pieces)
        .map(|piece| ClipSpec {
            // a file of the timeline's, not a mix under sound layers
            bus: None,
            path: piece.path.to_string_lossy().into_owned(),
            audio_stream: piece.stream.map(|index| index as u32),
            start: piece.start,
            duration: piece.duration,
            source_start: piece.source_start,
            volume: piece.volume as f32,
            volume_curve: piece
                .volume_curve
                .keys()
                .iter()
                .map(|key| GainKey {
                    at: key.at,
                    gain: key.value,
                    ease: [key.ease.x1, key.ease.y1, key.ease.x2, key.ease.y2],
                })
                .collect(),
            fade_in: piece.fade_in,
            fade_out: piece.fade_out,
            speed: piece.speed,
            preserve_pitch: piece.preserve_pitch,
            chain: piece.filter_chain,
        })
        .collect()
}

/// Tells `listener` about the player's failures from now on.
#[uniffi::export]
pub fn set_playback_listener(listener: Arc<dyn PlaybackListener>) {
    *events()
        .listener
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(listener);
}

/// Starts playing from `at_us`.
#[uniffi::export]
pub fn play(at_us: i64) -> Result<(), MobileError> {
    // A play pressed straight after a drag is not stopped by the drag's snippet.
    SNIPPET.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    SNIPPING.store(false, std::sync::atomic::Ordering::Release);
    player()?.play(seconds(at_us).max(0.0));
    Ok(())
}

/// Stops playing where it is.
#[uniffi::export]
pub fn pause() {
    SNIPPET.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    SNIPPING.store(false, std::sync::atomic::Ordering::Release);
    if let Some(player) = running() {
        player.pause();
    }
}

/// Bumped by every play, pause and drag: a scrub worker that finds it moved
/// under a play or a pause stops sounding.
static SNIPPET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Whether the player is sounding a snippet rather than playing.
static SNIPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Where the dragged playhead is now, microseconds; `i64::MIN` once taken.
static SCRUB_AT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(i64::MIN);
/// Whether a scrub worker is running.
static SCRUBBER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// How long a snippet sounds: long enough to tell a word or a beat, short
/// enough that a drag sounds like a tape under the head.
const SNIPPET_LENGTH: std::time::Duration = std::time::Duration::from_millis(90);

/// Sounds a moment of the timeline from `at_us` while the playhead is
/// dragged, as an editor's scrub audio does. Only the latest position is ever
/// sounded, a snippet at a time, by one worker that stops the player the moment
/// the drag stops: a drag's every move once queued a snippet of its own, and the
/// queue went on playing for seconds after the finger was still. Nothing while
/// a real play is on. Returns at once.
#[uniffi::export]
pub fn scrub_sound(at_us: i64) {
    use std::sync::atomic::Ordering;
    let Some(player) = running() else { return };
    if player.is_playing() && !SNIPPING.load(Ordering::Acquire) {
        return;
    }
    SCRUB_AT.store(at_us.max(0), Ordering::Release);
    if SCRUBBER.swap(true, Ordering::AcqRel) {
        return;
    }
    let mine = SNIPPET.load(Ordering::Acquire);
    let spawned = std::thread::Builder::new()
        .name("capcraft-scrub-sound".to_owned())
        .spawn(move || {
            loop {
                if SNIPPET.load(Ordering::Acquire) != mine {
                    // a play or a pause came: it owns the player now
                    break;
                }
                let at = SCRUB_AT.swap(i64::MIN, Ordering::AcqRel);
                if at == i64::MIN {
                    if SNIPPING.swap(false, Ordering::AcqRel) {
                        player.pause();
                    }
                    break;
                }
                SNIPPING.store(true, Ordering::Release);
                player.play(seconds(at));
                std::thread::sleep(SNIPPET_LENGTH);
            }
            SCRUBBER.store(false, Ordering::Release);
            // a drag that moved again as this worker was leaving is heard
            if SCRUB_AT.load(Ordering::Acquire) != i64::MIN
                && SNIPPET.load(Ordering::Acquire) == mine
            {
                scrub_sound(SCRUB_AT.load(Ordering::Acquire));
            }
        });
    if spawned.is_err() {
        SCRUBBER.store(false, Ordering::Release);
    }
}

/// Moves the playhead to `at_us`, playing or not.
#[uniffi::export]
pub fn seek(at_us: i64) {
    if let Some(player) = running() {
        player.seek(seconds(at_us).max(0.0));
    }
}

/// Where the player is now, in microseconds: the clock to draw by.
#[uniffi::export]
pub fn position_us() -> i64 {
    running().map_or(0, |player| micros(player.position_now()))
}

/// Whether it is playing.
#[uniffi::export]
pub fn is_playing() -> bool {
    running().is_some_and(|player| player.is_playing())
}

/// Silences the player (the editor's mute), or lets it be heard again.
#[uniffi::export]
pub fn set_muted(muted: bool) {
    if let Some(player) = running() {
        player.set_muted(muted);
    }
}

/// Lets the player go quiet with nothing loaded: the editor closed.
#[uniffi::export]
pub fn clear_playback() {
    if let Some(player) = running() {
        player.pause();
        player.set_clips(PathBuf::new(), Vec::new());
    }
}
