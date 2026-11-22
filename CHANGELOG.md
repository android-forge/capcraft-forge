# Changes from the original

CapCraft Forge is modified from CapCraft for Android at `31b4109` (4 November 2026, after v0.0.4-beta). Each change is
dated, as the licence asks; `#` numbers are the original project's.

## Unreleased

### 22 November 2026
- Opacity can be changed on keyframes, masks follow keyframes, and mask changes are no longer undone by the next (#46, #47; pull request #48).
- The volume slider follows keyframes (#49).
- Color match in Adjust (#58).
- A new project takes its first video's frame rate, and so does its export (#67; #65).
- The preview makes frames ahead and follows an even clock while playing (#68; #66), and a frame is copied less.
- An exported video begins with a keyframe (#45; #69).

### 23 November 2026
- A duplicate keeps every change, and stickers can be duplicated (#41; #70).
- The preview follows a picture being moved (#41; #71).
- 4K and ten-bit video plays from a proxy (egagofur's pull request #29, with its FFmpeg patch), and from its scrub copy until the proxy is written.
- Named CapCraft Forge, with an application id of its own. Exports go to folders of that name.
- Credits, licence texts and the source link of this build.
- My sounds in Sound FX: folders of sounds on the phone, by category.
- Filler words found and cut (#53); caption keywords in color (#54) and caption emojis (#55); captions with their English under them.
- Auto highlight keeps a video's liveliest moments.
- A project exported as a file and opened again (#56); a project used as a template.
- Preview quality (#57), clip groups (#59), compound clips of neighbouring videos (#60).
- HDR export in ten-bit HEVC (#61), with `src/scripts/ffmpeg-patches/0002-mediacodecenc-p010.patch`.
- A Home screen widget (#62), screen recording (#63), live photos in and out (#64).
- My voice: text read aloud in one's own voice, recorded saying it may be copied and checked to have said so.
- Graphs shape a mask's keyframes (#50). Titles paint the emoji of newer phones. A video can take a photo's place.
- Builds on a Mac as `mobile-env.sh` sets it up (#52). Photo is offered in every project's export.
- Remove objects: the people and things in a photo found, removed with the picture filled in behind, or put over the video; on
  main and overlay photos.
- Exposure in Adjust. Light people: people given the light they want, the rest left alone. Fine cutout: branches and other fine
  things cut from the sky. Atmosphere: overlays of the user's own, laid with a preset for their kind, behind people if asked.
- A much smaller app (38 MB, from 116): ONNX Runtime linked once, FFmpeg without what an editor of files never uses, the native
  libraries packed, the less-used fonts downloaded when wanted. The baseline profile is in, so it starts and scrolls smoothly.
- Photographs (CC0) in the Home screen's header, and README screenshots of this build.
