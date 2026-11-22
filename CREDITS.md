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

Fonts not in the app are downloaded when wanted from Google Fonts' repository (SIL Open Font License; Kosugi Maru Apache-2.0).
Remove objects uses YOLOX (Megvii, Apache-2.0), SlimSAM (Apache-2.0) and MI-GAN (Picsart AI Research, MIT), each downloaded
when it is first used.

## Changes in this build

4K playback from proxies, and ten-bit HEVC on the phone's video chip, are by egagofur
(artcraft-android/capcraft-android#29), offered to the original project and not merged there yet; the commits are egagofur's.
FFmpeg is built with that pull request's patch, `0001` in `src/scripts/ffmpeg-patches`; `0002` there, ten-bit encoding on the
phone's video chip, is this build's own, under FFmpeg's licence (LGPL-2.1-or-later).

The rest is by praveenkrishna16n-sys, with Claude Code, in November 2026; see [CHANGELOG.md](CHANGELOG.md). Most of the fixes are
offered to the original project as pull requests.

Thanks to those who reported what they fix: Class9123 (artcraft-android/capcraft-android#45) and XPERTER-sysnand (#41).

## Photographs and film

The photographs on the Home screen and in the README's screenshots come from Wikimedia Commons and are
[CC0](https://creativecommons.org/publicdomain/zero/1.0/), made smaller and cropped here. CC0 asks for no credit; their
photographers deserve it:

- [Lofoten, Norway (Unsplash)](https://commons.wikimedia.org/wiki/File:Lofoten,_Norway_(Unsplash).jpg): Johannes Groll
- [Alone in the unspoilt wilderness (Unsplash)](https://commons.wikimedia.org/wiki/File:Alone_in_the_unspoilt_wilderness_(Unsplash).jpg): David Marcu
- [Juan Griego Sunset](https://commons.wikimedia.org/wiki/File:Juan_Griego_Sunset.jpg): Wilfredor
- [Hong Kong skyscrapers in a night of typhoon](https://commons.wikimedia.org/wiki/File:Hong_Kong_skyscrapers_in_a_night_of_typhoon.jpg): Wilfredor
- [Lotus flower (978659)](https://commons.wikimedia.org/wiki/File:Lotus_flower_(978659).jpg): Hong Zhang (jennyzhh2008)
- [Eisvogel kingfisher](https://commons.wikimedia.org/wiki/File:Eisvogel_kingfisher.jpg): Frank-2.0
- [Price Building illuminated at night in Quebec City](https://commons.wikimedia.org/wiki/File:Price_Building_illuminated_at_night_in_Quebec_City.jpg): Wilfredor
- [Common Jezebel Delias eucharis by kadavoor 3](https://commons.wikimedia.org/wiki/File:Common_Jezebel_Delias_eucharis_by_kadavoor_3.jpg): Jeevan Jose

Two of the README's screenshots (the film clip, and its project on Home) show a frame of [Big Buck Bunny](https://peach.blender.org), © 2008 Blender Foundation,
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/), as the app shows it. The film is not part of this app.

Neither CapCraft nor CapCraft Forge is affiliated with, endorsed by or connected to ByteDance or CapCut.
