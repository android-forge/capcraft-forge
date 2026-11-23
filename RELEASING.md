# Releasing this build

Not legal advice. Work through this for every release.

## Before the first release
- [ ] Ask the original authors (see the issue this build's maintainer opens there).
- [x] A name: CapCraft Forge. It still has CapCraft in it: ask the original authors, or change it.
- [ ] An icon of its own, and no use of CapCut's.
- [x] An application id of its own in `app/build.gradle.kts`: another key cannot update the original's app.
- [x] A signing key of its own, backed up, never committed; its values as the release workflow's secrets.
- [x] README images and links that are this build's, and not the original's store badges.
- [ ] Make this repository public, or point `SOURCE_URL` in `HomeScreen.kt` at a public one: whoever gets the APK must get its source.
- [ ] Ask Concat whether its model mirror may be used by this build.

## Every release
- [x] Set a version of its own: 0.0.5 (version code 599), after the original's 0.0.4-beta, so it installs over it.
- [ ] Update CHANGELOG.md with what changed and when.
- [ ] Run `src/scripts/licences.py <ffmpeg source>` and commit what it changes.
- [ ] Credit anyone whose work was added in CREDITS.md, and keep their commits as theirs.
- [ ] Tag the very commit the APK is built from, and push the tag before publishing the APK.
- [ ] Attach to the release, beside the APK: the source at the tag; FFmpeg's source archive and any patch; the sources of
      sherpa-onnx at the version in Cargo.lock and of the espeak-ng it builds. The links may move; the licence's duty does not.
- [ ] Test the release build (only debug builds have been tested) on a phone: import, edit, play, export, and About and Licences.
