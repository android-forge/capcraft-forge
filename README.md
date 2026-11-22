> **CapCraft Forge is an unofficial, modified build of [CapCraft for Android](https://github.com/artcraft-android/capcraft-android)**
> by the ArtCraft Team, built on [Concat](https://github.com/jub0t/Concat) by Jareer Abdullah and the Concat contributors.
> All credit for the original work goes to them. They do not maintain or endorse this build; please report its problems here.
> See [CREDITS.md](CREDITS.md) and [CHANGELOG.md](CHANGELOG.md). Not affiliated with ByteDance or CapCut.

<p align="center">
  <img src="docs/images/capcraft-512.png" alt="CapCraft's app icon" width="128">
</p>

<h1 align="center">CapCraft Forge</h1>

<p align="center">
  <b>An open-source video editor for Android.</b><br>
  Many credits to the original maintainers, <a href="https://github.com/jub0t">jub0t</a> and the Concat contributors.
</p>

<p align="center">
  <img alt="Rust engine" src="https://img.shields.io/badge/engine-Rust-lightblue?style=flat&logo=rust&logoColor=white">
  <img alt="Kotlin and Jetpack Compose" src="https://img.shields.io/badge/UI-Kotlin%20%C2%B7%20Jetpack%20Compose-blue?style=flat">
  <img alt="Minimum: Android 10 (API 29)" src="https://img.shields.io/badge/min-Android%2010%20%28API%2029%29-blue?style=flat">
  <img alt="Target: Android 16 (API 36)" src="https://img.shields.io/badge/target-Android%2016%20%28API%2036%29-blue?style=flat">
  <img alt="License: AGPL-3.0-or-later" src="https://img.shields.io/badge/license-AGPL--3.0--or--later-blue?style=flat">
  <img alt="Status: beta" src="https://img.shields.io/badge/status-beta-lightblue?style=flat">
</p>

<p align="center">
  <a href="#editing">Editing</a> ·
  <a href="#color">Color</a> ·
  <a href="#filters-effects-and-transitions">Filters, effects &amp; transitions</a> ·
  <a href="#text-and-stickers">Text &amp; stickers</a> ·
  <a href="#sound-and-export">Sound &amp; export</a> ·
  <a href="#planned-features">Planned features</a> ·
  <a href="#building-instructions">Building</a>
</p>

<br>

<table align="center">
<tr>
<td align="center"><img src="docs/images/home.png" alt="Home: photographs drifting in the header, New video and Edit photo, three recent projects, and the tools AutoCut, Retouch, Photo tools, Shoot and record, Auto enhance and Cover maker" width="250"></td>
<td align="center"><img src="docs/images/editor.png" alt="The editor: a photograph of northern lights with the title Northern Nights, the transport row, and a timeline with a main track, a transition at the cut and a text row, over the tools Edit, Audio, Text, Effects, Overlay and Captions" width="250"></td>
</tr>
<tr>
<td align="center"><sub>Home: start a video or a photo, then your projects</sub></td>
<td align="center"><sub>The editor: preview, timeline and tools</sub></td>
</tr>
</table>

<br>

## Editing

<table align="center">
<tr>
<td align="center"><img src="docs/images/clip-tools.png" alt="A clip selected on the main track with its trim handles and length, and clip tools such as Duplicate, Replace, Overlay, Adjust and Filters" width="250"></td>
<td align="center"><img src="docs/images/mask.png" alt="The Mask panel with Circle chosen: the photograph shown through a circle on the preview, with handles to size it, and None, Horizontal, Mirror, Circle and Rectangle" width="250"></td>
<td align="center"><img src="docs/images/film.png" alt="The editor with a frame of the film Big Buck Bunny on the preview and its clip on the timeline" width="250"></td>
</tr>
<tr>
<td align="center"><sub>A clip selected: trim, duplicate, replace, adjust and more</sub></td>
<td align="center"><sub>Masks, moved and sized on the preview</sub></td>
<td align="center"><sub>Video clips on the timeline</sub></td>
</tr>
</table>

<br>

## Color

<table align="center">
<tr>
<td align="center"><img src="docs/images/adjust.png" alt="Adjust, Customize: the smart tools Auto adjust, Color correction and Color match, then Brightness, Contrast and Saturation, with a slider" width="250"></td>
<td align="center"><img src="docs/images/color-match.png" alt="Color match: the aurora photograph taking the warm colors of a sunset frame, with the frame chosen by a slider, Match this frame, Photo, and a Strength slider" width="250"></td>
</tr>
<tr>
<td align="center"><sub>Adjust: smart tools and sliders</sub></td>
<td align="center"><sub>Color match: take the colors of another frame or a photo</sub></td>
</tr>
</table>

<br>

## Filters, effects and transitions

<table align="center">
<tr>
<td align="center"><img src="docs/images/filters.png" alt="The Filters panel: None, Fresh, Vivid and Clean, each a preview of the clip itself, on shelves such as Everyday, Portrait and Cinematic" width="250"></td>
<td align="center"><img src="docs/images/effects.png" alt="The Video effects panel: shelves such as Motion, Light, Distort, Retro and Stylize, and effects such as Shake, Flash, Pulse, Glitch, VHS, Old Film and Rainbow, each previewed on the frame" width="250"></td>
<td align="center"><img src="docs/images/transitions.png" alt="The Transitions panel at a cut: None, Dissolve, Blur Dissolve, Fade to Black, Fade to White, Film Burn, Zoom, Slide, Push, Spin, Swirl and Glitch, each tile drawn with the clips" width="250"></td>
</tr>
<tr>
<td align="center"><sub>80+ filters, previewed on the clip</sub></td>
<td align="center"><sub>Video effects, on a clip or on a row of their own</sub></td>
<td align="center"><sub>Transitions, shown on your own clips</sub></td>
</tr>
</table>

<br>

## Text and stickers

<table align="center">
<tr>
<td align="center"><img src="docs/images/text.png" alt="The text sheet for Northern Nights on its Styles tab, beside Templates, Fonts, Effects, Animations and Bubbles, with colors, size and opacity" width="250"></td>
<td align="center"><img src="docs/images/stickers.png" alt="The Stickers panel on its shapes shelf: square, circle, triangle, line, arrow, star, heart and hexagon" width="250"></td>
</tr>
<tr>
<td align="center"><sub>Text: templates, fonts, styles, effects, animations and bubbles</sub></td>
<td align="center"><sub>Stickers, labels, shapes and doodles</sub></td>
</tr>
</table>

<br>

## Sound and export

<table align="center">
<tr>
<td align="center"><img src="docs/images/sounds.png" alt="The Sound FX panel: a search field, the kinds Swooshes, Pops, Bells, Booms and Game, and sounds such as Whoosh, Swoosh down, Whip and Riser, each with its length and an add button" width="250"></td>
<td align="center"><img src="docs/images/export.png" alt="The export settings: Video and GIF tabs, and sliders for resolution (480p to 4k, set at 1080p), frame rate (24 to 60, set at 30) and bitrate, with the estimated file size" width="250"></td>
</tr>
<tr>
<td align="center"><sub>Sound effects, made by the app</sub></td>
<td align="center"><sub>Export at 480p to 4K, 24 to 60 fps, or as a GIF or photo</sub></td>
</tr>
</table>

<br>

<p align="center"><sub>Every picture here is a screenshot of the app. The photographs in them are CC0; Big Buck Bunny is © Blender Foundation, CC BY 3.0. See <a href="CREDITS.md#photographs-and-film">CREDITS.md</a>.</sub></p>

<br>

## Planned features

CapCraft Forge is in beta.

| Area | Status |
|---|---|
| Projects from the phone's videos and photos, or shared to the app; autosave; rename, duplicate, delete | ✅ |
| The timeline: trim, move, split, delete, duplicate, replace, multi-select, undo and redo; pinch to zoom | ✅ |
| Speed and curves, freeze frame, reverse, volume and fades, crop, mask, chroma key, keyframes | ✅ |
| Overlays and layers with blend modes, opacity, transform and animations | ✅ |
| Filters, adjustments (HSL, curves, colour wheels), color match, video effects, body effects and transitions | ✅ |
| Text: templates, fonts, styles, word art, bubbles, animations and text to speech | ✅ |
| Stickers, labels, shapes and drawing | ✅ |
| Captions made on the phone, typed captions, caption templates and imported subtitles | ✅ |
| Sound effects, voiceover, extract audio, voice effects, noise reduction and isolate voice | ✅ |
| Remove background; find people and things in a photo, remove them or put them in an overlay (models downloaded once, run on the phone) | ✅ |
| Aspect ratios, backgrounds, cover, AutoCut, auto highlight, collage, projects as templates | ✅ |
| Filler words removed, caption keywords and emojis, English under captions, one's own voice | ✅ |
| Groups, compound clips, project files, screen recording, live photos, a Home widget, one's own sound folders | ✅ |
| Export at 480p to 4K, 24 to 60 fps, HDR, GIF, photo and live photo, saved to the gallery and shared | ✅ |
| A music library, downloadable effects and templates | ⬜ |
| Generative AI, cloud storage, accounts | 🚫 CapCraft is a local app |

<sub>✅ works today · 🚧 in progress · ⬜ not started · 🚫 not planned</sub>

<br>

## Building Instructions
*One Gradle command builds the app: it compiles the Rust engine with [cargo-ndk](https://github.com/bbqsrc/cargo-ndk) and generates the Kotlin bindings. FFmpeg and sherpa-onnx are fetched and built first, once.*

* Clone this repository
* Make sure you have at least 10 GB of free disk space
* In `src`, with `ANDROID_HOME` pointing at your Android SDK:
  ```
  scripts/ffmpeg-mobile.sh aarch64-linux-android
  scripts/sherpa-mobile.sh aarch64-linux-android
  eval "$(scripts/mobile-env.sh aarch64-linux-android)"
  ```
* then enter `src/crates/capcraft-android/android` and run `./gradlew assembleRelease`.

Build time dependencies:

<details>
  <summary>required tools</summary>

  ```
      Linux or macOS (on Windows, WSL)
      rustup (it installs the Rust release rust-toolchain.toml names)
      rustup target add aarch64-linux-android
      cargo install cargo-ndk --version 4.1.2 --locked
      Android SDK (API 37) and NDK 29.0.14206865
      JDK 17 or newer
      cmake, make, clang (libclang-dev), curl
  ```
</details>

*Builds are only tested in WSL Ubuntu on Windows 11.*

<br>

## License

[AGPL-3.0-or-later](LICENSE). CapCraft Forge is not affiliated with, endorsed by or connected to ByteDance or CapCut.

<br>

## Misc info for myself

Try to use [semantic commit](https://kapeli.com/cheat_sheets/Semantic_Commits.docset/Contents/Resources/Documents/index)
messages. Use `feat:` / `fix:` / `chore:` / `docs:` / etc.
