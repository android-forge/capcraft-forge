# Credits

CapCraft Forge is an unofficial, modified build of [CapCraft for Android](https://github.com/artcraft-android/capcraft-android).
Its authors do not maintain or endorse this build. Please report its problems in this repository, not in theirs.

## The original work

- **CapCraft for Android**: the ArtCraft Team and the CapCraft contributors, AGPL-3.0-or-later. Nearly all of this app is
  theirs. The commits in this repository's history name each author.
- **[Concat](https://github.com/jub0t/Concat)**, the engine under it: Jareer Abdullah (jub0t) and the Concat contributors,
  AGPL-3.0-or-later.

## What they are built on

FFmpeg (LGPL-2.1-or-later); whisper.cpp and ggml (MIT); ONNX Runtime (MIT); sherpa-onnx (Apache-2.0), which carries
espeak-ng (GPL-3.0-or-later) and piper-phonemize (MIT); UniFFI (MPL-2.0); JNA, Kotlin, AndroidX and Jetpack Compose
(Apache-2.0); Lucide icons (ISC); Material Symbols (Apache-2.0); the fonts; and the models downloaded on request. Who made
each, and its licence, is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md),
[NOTICE.md](src/crates/capcraft-android/NOTICE.md) and [licences/](licences).

## Changes in this build

4K playback from proxies, and ten-bit HEVC on the phone's video chip, are by egagofur
(artcraft-android/capcraft-android#29), offered to the original project and not merged there yet; the commits are egagofur's.
FFmpeg is built with that pull request's patch, in `src/scripts/ffmpeg-patches`.

The rest is by praveenkrishna16n-sys, with Claude Code, in November 2026; see [CHANGELOG.md](CHANGELOG.md). The fixes are offered to the
original project as pull requests.

Thanks to those who reported what they fix: Class9123 (artcraft-android/capcraft-android#45) and XPERTER-sysnand (#41).

Neither CapCraft nor CapCraft Forge is affiliated with, endorsed by or connected to ByteDance or CapCut.
