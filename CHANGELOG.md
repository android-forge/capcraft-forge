# Changes from the original

CapCraft Forge is modified from CapCraft for Android at `31b4109` (4 November 2026, after v0.0.4-beta). Each change is
dated, as the licence asks; `#` numbers are the original project's.

## Unreleased

### 28 November 2026
- Engine from Concat 0.2.13: 22 new looks (a Flash shelf - Flash, Strobe, Flicker, Beat Pulse, RGB Flash; Glitch, VHS
  Tracking, Old Film, Lens Flare, Split Screen, Mirror, Kaleidoscope, Blur Frame, Light Leak, Zoom Pulse, Duotone; the
  transitions Flash, Luma Wipe, Doors, Blinds, Mosaic and Ripple), Concat's rendering and decoding fixes (a failed
  frame is drawn again rather than losing the device; video decoders held to what the graphics memory has room for),
  pitch-kept time stretching, and the studio voice about seven times faster (its models run in int8). Forge keeps
  its own project format, export and voices: 0.2.13 replaces Concat's project model, which Forge is built on.

### 27 November 2026
Engine updated to [Concat v0.2.12](https://github.com/jub0t/Concat/releases/tag/v0.2.12) by Jareer Abdullah (jub0t)
and Concat's contributors, from the v0.2.8 base merged on 28 October (v0.2.9, v0.2.10, v0.2.11 and v0.2.12; release
notes at https://github.com/jub0t/Concat/releases). Where Forge and Concat had each built the same thing, the better one
was kept:
- From Concat: exports drawn ahead and handed to the encoder on its own thread as YUV (about 2.4× faster where the
  Android surface encoder is not used), the encoder stall and black-frame fixes at cuts and joins (#747, #756, #790),
  variable-frame-rate and screen recordings played frame by frame (#457), GPU-loss recovery during export, colour emoji
  painted from `COLR` v1 with each picture cached, editable shapes (width, height, corners, gradients, outlines, dashes,
  shadows, trim; Forge's Hexagon and Diamond are now six- and four-sided polygons), stacked masks per clip, caption
  pruning of made-up lines over silence and music, keyframe-aware nearest-frame seeks, and enhance at any scale up to 2K.
- Kept from Forge: the direct Android MediaCodec surface export, Kokoro, Pocket and Supertonic voices (Concat removed
  them for Chatterbox, which is desktop only), a picture stream chosen past a cover image, per-word caption timings,
  and adjustment layers that know their own length.
- Looks Concat renamed (Polaroid, Rain, Milk and eleven others) still open under their old names in older projects.
- Concat v0.2.13 is not merged yet: it replaces the project model with a new document crate, which Forge's Android code
  has to be ported to first.
- Reel FX settings by kind: sliders for numbers, colour dots, a line-by-line editor for lists, chips for looks, shapes
  and every app font; tap a selected effect on the preview to edit its words there, and drag Swipe's line, The Tap,
  Wave Trim, Pause & Point's label, Reject Stamp and Messages into place. A new effect shows itself as soon as it is
  added, and a Sound row adds one of the built-in sounds at its start. Reel Frame has the full icon column with counts;
  Countdown Flip has looks; Scribble has shapes; Spotlight has light presets; Fan Build, Tape Label and Glass Widgets
  are redrawn; Match Cut takes your own pictures; Carousel Spin's near panels pass in front of you.
- Atmosphere: the filter row is gone and the settings scroll, so the light sliders no longer overlap.
- Reel FX on the timeline: Effects > Reel FX lays each effect on the effects row as an item of its own, drawn on the
  pictures beneath it - move it, trim or extend it, and key its opacity to fade it in and out. Each effect shows only
  the settings it reads (Swipe's after look and Backdrop Word's font as choices), not a generic words box.
- Film looks (in Filters, under Film): 13 film stocks and grades - Vision3 500T and 250D, Eterna 500, Ektachrome 100,
  Delta 3200, CineStill 800T, Kodak Gold, teal and orange, bleach bypass, night bloom, documentary, day for night and
  cross process - each with an amount, after 0xzgbot's color-grading-film-emulation skill (MIT).
- Reel FX (in Look): 51 talking-head effects - orbit, spotlight, backdrop word, glass widgets, LED sign, checklist,
  tier list, match cut and the rest - ported from Vibha Ramprakash's claude-video-effects (MIT). Each is placed on a
  stretch of the clip, follows the face (OpenCV's YuNet, built in) and can sit behind the person (Remove BG's person
  model); the face and the person are found on the phone the first time, with the progress in the panel. Effects with
  pictures use the clip's own frames, or images you pick. Preview and export draw the same frames; a split keeps each
  half's effects.
The editor's tools reorganised (the first three steps of `docs/proposals/app-cleanup-and-tools.md`). Nothing was removed.
- A picture's bar is Split, Speed, Volume, Duplicate and Delete, always in the same places, then five drawers: **Edit**,
  **Look**, **Smart** (the models and heavy processing), **Motion** and **Sound**. That's 10 tools where there were
  about 37 in one row. The drawer last used stays open for the next picture picked.
- Text, stickers, captions and sounds have short bars of their own with a **More** drawer (a sound's are **Smart** and
  **Clean**).
- The top level is Edit, Audio, Text, Captions, Effects, Overlay, Color, Background, Ratio and Project. Stickers and
  Graphics are only under Text, Auto captions and lyrics only under Captions, Filters and Adjust are one Color, and
  Transcript, Filler words, Auto highlight and Replace media are under Project. Preview quality moved to a ⋮ menu in
  the top bar.
- Renamed for what they do: Splice is Blend, Maintrack and Overlay are Move to main and Move to overlay, Graphs is
  Keyframes, Basic is Position, Advanced is Exact values, Video quality is Enhance (Photo quality on photos), the Audio
  group's Extract is From a video, and in Adjust, Graphs is Curves and Hue is Tint.
- Adjust's knobs are grouped Light, Color, Detail and Curves, with a tab for each group that jumps to it and shows a
  dot when anything in the group is changed.
- Search finds tools inside the drawers, and by their old names too. A one-time tip says the tools have moved.
- The search sheet's "Reduce video noise" suggestion, which turned on the sound's noise reducer, now says "Reduce
  background noise".
- The effect browser no longer offers Vignette and Film grain, which are Adjust's knobs.
- Retouch finds skin with a model: Google's MediaPipe Selfie Multiclass Segmentation (Apache-2.0), converted to ONNX
  and built into the app, so nothing is downloaded. Once Retouch is on a photo or video, the face's and the body's skin
  are found on the phone, ten looks a second, kept with the project like Remove BG's masks, with "Finding skin" and its
  progress in the panel. Skin-coloured walls, wood and sand are no longer smoothed, and darker skin that the colour
  guess missed is. Until a stretch has been looked at, Retouch goes by colour as before.

## 0.0.8 (24 November 2026)

### 28 November 2026
- Collage is a home tool of its own: pick two to nine photos or videos and the collage opens on its own screen - the
  picture, and Layout, Borders and Frame under it - with the whole editor a tap away ("Edit in timeline").
  Layout adds a picture (+) or takes the last off (-) and swaps two pictures with two taps on the cell map; Borders
  sets the gap's width, the corners' roundness and the gap's colour; Frame makes it 1:1, 4:5, 3:4, 9:16 or 16:9.

### 24 November 2026
Ideas from [FilmCraft](https://github.com/storytold/filmcraft) (Apache-2.0), written here for a phone.
- Export presets for Instagram Reels (1080p, 30 fps), TikTok (1080p, 30 fps) and YouTube Shorts (1080p, 60 fps), with a
  note when the project is not 9:16.
- Split scenes: a video clip cut wherever its shot changes, read on the phone by the shot reader Reel to template uses,
  in one undo step.
- Save LUT in Adjust: a clip's grade drawn over a 33-step lattice on the GPU and written as a `.cube`, kept in Looks and
  handed to the share sheet; sharpen, vignette, clarity and grain are left out and named, as a LUT cannot hold them.
- Scopes in Adjust: a histogram, a waveform and a vectorscope with a skin-tone line over the preview, read from the
  picture five times a second; a tap moves to the next.
- The timeline is heard under a dragged playhead, in snippets of the latest place only, and not while it races or a
  fling coasts.
- Auto duck: a music clip turned down 12 dB wherever the other clips speak, with volume keys that fade down and up.
- Fit to video: a song made to end with the video, a stretch of its middle taken out or played again with both ends of
  the join on a beat. The join is worked out before anything changes, so a song that cannot be joined is left as it was.

## 0.0.7 (24 November 2026)

### 24 November 2026
- Android preview can decode supported SDR H.264 and HEVC proxy frames directly into GPU buffers and import them into Vulkan/wgpu without a CPU pixel round trip. Cutouts use a separate cached GPU matte, including crop and flips; stabilized cutouts and the brush-highlight preview keep their CPU pixel path.
- Android export can render SDR AVC/HEVC and capability-supported 10-bit HEVC SDR/HDR (PQ/HLG) directly to a MediaCodec input surface, avoiding GPU readback before encoding. Unsupported devices and alpha codecs retain the existing encoder fallback.
- Decoder seats are managed during playback and scrub work yields so background reads do not crowd out the playing clip.
- Ninety locally generated atmosphere overlays across fifteen categories, with favorites, recent items, side-by-side comparison and a gentle motion action. No bundle artwork is embedded. The overlay being laid shows as chosen at once, and the panel is never left busy if laying it fails.
- Custom speed curves can add any number of interior points by tapping the graph, then dragging each point to shape time and speed.

## 0.0.6 (23 November 2026)

### 24 November 2026
- Custom background removal tools are labelled Quick brush, Brush, Quick erase and Eraser.
- Brush size is adjustable from 1% to 50%, with the selected percentage shown on the slider.
- The live brush preview follows a continuous stroke with rounded ends and joins.

### 23 November 2026
- An icon of its own: a white CF, the C's top running on into the F, and a spark struck off it, on a plum-to-ember
  tile; one colour for themed icons. Its source is `docs/images/forge-icon.svg`. CapCraft's camera is gone.
- Playback first: the copies written after an import (the scrub copy every video gets, the proxy a large one does)
  stop while the player plays and give their decoder back until it stops. A 4K clip played at one or two frames a
  second while its copy was written, and at sixty alone.
- A copy of a video larger than 720p is decoded on the phone's video chip, on two threads where it cannot be; each
  video's proxy is written first and its scrub copy made from it, so a 4K file is read once, not twice.
- "Preparing clips · 3 of 11 ready" over the preview while the copies are written.
- A new project is as sharp as the sharpest clip put on with it (1080 on the short side at most): a 480p clip first
  no longer makes a 480p project. The export asks by default for the sharpest clip's size, so 4K clips leave as 4K.
- A prototype of zero-copy video (`src/crates/zerocopy-proto`, not part of the app): a phone's decoded frames
  imported into Vulkan and wgpu with no copy through the CPU, at 159-162 frames a second of 4K against 45-47 on the
  road the editor takes today.

## 0.0.5 (23 November 2026)

The first release of CapCraft Forge.

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
- A much smaller app (38 MB, from 116; 43 with the graphics): ONNX Runtime linked once, FFmpeg without what an editor of files never uses, the native
  libraries packed, the less-used fonts downloaded when wanted. The baseline profile is in, so it starts and scrolls smoothly.
- Speech that tells "live" in "broadcast live" from "how you live": Supertonic 3 voices, and words spelt as they sound for the
  voices that look them up.
- Graphics: 20,236 emoji, stickers, icons, flags, logos and shapes for video, kept in the app and found by words without the
  internet; `graphics/build.py` packs them. Timing: when a graphic, title or overlay comes on and goes.
- Stock photos, full HD or better, searched on the import page: Wikimedia Commons, NASA and Unsplash (with a key of one's own).
  Stock videos beside them, narrowed to any of twelve languages of India: public domain films from 1965 and before on the
  Internet Archive, and Commons videos; only what may be changed and used commercially.
- Beats: the main track's clips cut so each change lands on a beat, or all made one length, in one tap.
- Atmosphere's overlays (smoke, clouds, lights, rays) scaled, turned, moved along either axis and flipped upside down. Light
  laid in 3D reads the photo's depth (Depth Anything V2 Small, fetched on first use), puts the scene back in 3D, and traces
  from the light's brightest point what each part of it would be shaded by, so shadows fall with the scene's own shapes and
  sizes; they fade where the photo's own lamps already light it, are as dark as the light is strong, and take the colour of
  the light still reaching in. The people are lit from the light's side in its colour (Relight), and the shadow is traced
  again when the light is scaled, turned or moved. 2D lays it flat, as before. The last Golden light settings (glow 80,
  warm photo 66) are the defaults.
- People in colour: Body effects turn the people grey (or, inverted, all but them), or tint them (angry red, cold blue, sick green), with the amount
  set and grown from the clip's start to the playhead. A Tint effect does it.
- Remove background: Invert keeps the background and takes the person away.
- People effects and Atmosphere overlays on a photo now lie over the clip they are for, rather than at the end of the
  timeline, when another overlay is further along.
- Reel to template: a finished video's cuts, transitions, moves, flashes and sound read on the phone and laid out, for other media
  to go in. A video shared to the app may be made one.
- Golden light in Atmosphere: rays, lights and window beams yellow, and the picture warmed by them; the colour (golden,
  sunset, warm white, pink, blue hour or the overlay's own), the glow and the warmth are the user's to set. An overlay's black
  is taken away before it is brightened, so it leaves the picture under it as it was.
- Manners for text to audio: an epic narrator, a storyteller, an announcer and a calm voice.
- Reel to template reads more (tested on the phone): the words it shows (PaddleOCR, downloaded when wanted) as titles in their colour, size and
  outline; its voice and music on rows of their own; whether its pictures sit between bars or over a blur of themselves; and
  the media put in it take the colours of its own.
- Light people gives a face in shadow a soft light of its own (`capcraft.face-light`), so dark clothes stay dark.
- Titles and graphics can go behind the person; graphics take effects; modern animations with a reel's easing; reel title
  styles in twelve new faces (SIL Open Font License).
- The README shows what is this build's own.
- Built-in templates on Home: fifteen reel looks (Year Roll, Day Count, Halo Circle, Frame and Heart, Depth Slide, Split
  Reveal, Wide Roll, Velocity, Flash & Shake, Glow Rush, Seamless Zoom, Into the Eye, Beat Flash, Encore), each with places for one's own
  photos and videos and titles to type over. Encore ramps, rewinds, mirrors and echoes the user's own clips: a place
  can copy another and fill with it, and a template can bring public domain sample photos and music. Stock photos are tried again when a site asks for a pause.
- Photographs (CC0) in the Home screen's header, and README screenshots of this build.
