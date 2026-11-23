// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! A prototype of zero-copy video on a phone, run from a shell over adb:
//!
//! ```text
//! zerocopy-proto <video.mp4> [cpu|gpu|both] [frames]
//! ```
//!
//! It decodes one file twice on the phone's hardware decoder and times each
//! road to the same place - an RGBA picture of the preview's size in a wgpu
//! texture, which is what the compositor draws from:
//!
//! - **cpu**, the editor's road today: the decoder writes each picture into
//!   memory, the CPU converts and shrinks it with swscale, and it is uploaded.
//! - **gpu**, zero-copy: the decoder writes into a buffer the GPU reads as it
//!   is (an `AHardwareBuffer` from an `AImageReader`), imported into Vulkan,
//!   converted from Y'CbCr and shrunk by one compute pass, into an image wgpu
//!   adopts.
//!
//! Each road reports its frames a second, unpaced, and the CPU it took: this
//! process's own and the whole phone's, since the decoder runs in a process of
//! its own. The first picture of each is written beside the binary as a PPM to
//! compare by eye.

#[cfg(target_os = "android")]
mod phone;

#[cfg(target_os = "android")]
fn main() {
    if let Err(error) = phone::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
    eprintln!("zerocopy-proto runs on an Android phone: build it for aarch64-linux-android");
    std::process::exit(2);
}
