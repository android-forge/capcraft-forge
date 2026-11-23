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
- A much smaller app (38 MB, from 116; 43 with the graphics): ONNX Runtime linked once, FFmpeg without what an editor of files never uses, the native
  libraries packed, the less-used fonts downloaded when wanted. The baseline profile is in, so it starts and scrolls smoothly.
- Speech that tells "live" in "broadcast live" from "how you live": Supertonic 3 voices, and words spelt as they sound for the
  voices that look them up.
- Graphics: 20,236 emoji, stickers, icons, flags, logos and shapes for video, kept in the app and found by words without the
  internet; `graphics/build.py` packs them. Timing: when a graphic, title or overlay comes on and goes.
- Stock photos, full HD or better, searched on the import page: Wikimedia Commons and NASA, and Pexels and Unsplash with a key of
  one's own. Stock videos and meme pictures beside them, narrowed to any of twelve languages of India: public domain films from
  1965 and before on the Internet Archive, Commons, Openverse, and Pexels and Pixabay with a key; only what may be changed and
  used commercially (public domain, CC0, CC BY credited, and the sites' own licences). Among the memes, GIPHY GIFs (silent
  videos, credited, "Powered by GIPHY", with a key of one's own) and Imgflip's most used templates, whose makers keep their
  rights: their use is the user's to judge. Pexels videos among the memes too. tamilmemes.co.in, indianmemetemplates.com,
  memes.co.in, videomemes.in and memestamil.in open in the browser, as their downloads go through their own pages. YouTube and Klipy are left out, as their
  API terms forbid keeping copies.
- Beats: the main track's clips cut so each change lands on a beat, or all made one length, in one tap.
- Atmosphere's overlays (smoke, clouds, lights, rays) scaled, turned, moved along either axis and flipped upside down.
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
