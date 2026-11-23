// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Zero-copy frames: a picture a platform's video decoder wrote into GPU
//! memory, turned into the compositor's own picture on the GPU, never copied
//! through the CPU.
//!
//! The decoder (in `concat-media`) and the compositor (in `concat-render`)
//! know nothing of each other, so they meet here. The compositor installs an
//! [`Importer`] for its device once it has one that can read such buffers;
//! a decoder that has a buffer asks [`importer`] for it and, when there is
//! one, hands the buffer over and wraps the answer with
//! [`crate::frame::Frame::on_gpu`]. With no importer there are no such
//! frames, and every decoder reads the way it always did.

use std::any::Any;
use std::ffi::c_void;
use std::sync::{Arc, OnceLock};

/// Turns a decoder's hardware buffer into a picture on a GPU.
pub trait Importer: Send + Sync {
    /// `buffer` (an Android `AHardwareBuffer`, borrowed for the call), whose
    /// picture is the `crop` rectangle `[x, y, width, height]` of it, shown
    /// upright once turned `rotation` degrees clockwise (0, 90, 180 or 270),
    /// as an eight-bit RGBA picture `size` across and down on the importer's
    /// GPU, finished by the time this returns - the buffer may go back to the
    /// decoder at once. `None` when it cannot, and the decoder falls back.
    fn import(
        &self,
        buffer: *mut c_void,
        crop: [u32; 4],
        rotation: u32,
        size: (u32, u32),
    ) -> Option<Arc<dyn Any + Send + Sync>>;
}

static IMPORTER: OnceLock<Arc<dyn Importer>> = OnceLock::new();

/// Makes `importer` the one decoders hand their buffers to, if none is yet:
/// the first compositor that can is the preview's, made when the engine
/// starts. Returns whether this one was installed.
pub fn install(importer: Arc<dyn Importer>) -> bool {
    IMPORTER.set(importer).is_ok()
}

/// The installed importer, if a compositor has installed one.
pub fn importer() -> Option<&'static Arc<dyn Importer>> {
    IMPORTER.get()
}
