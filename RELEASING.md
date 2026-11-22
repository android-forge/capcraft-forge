# Releasing this build

Not legal advice. Work through this for every release.

## Before the first release
- [ ] Ask the original authors (see the issue this build's maintainer opens there).
- [ ] A name and an icon of its own, and no use of CapCut's. Until then this is still called CapCraft.
- [ ] An application id of its own in `app/build.gradle.kts`: another key cannot update the original's app.
- [ ] A signing key of its own, backed up, never committed; its values as the release workflow's secrets.
- [ ] README images and links that are this build's, and not the original's store badges.
- [ ] Ask Concat whether its model mirror may be used by this build.

## Every release
- [ ] Update CHANGELOG.md with what changed and when.
- [ ] Run `src/scripts/licences.py <ffmpeg source>` and commit what it changes.
- [ ] Credit anyone whose work was added in CREDITS.md, and keep their commits as theirs.
- [ ] Tag the very commit the APK is built from, and push the tag before publishing the APK.
- [ ] Attach to the release, beside the APK: the source at the tag; FFmpeg's source archive and any patch; the sources of
      sherpa-onnx at the version in Cargo.lock and of the espeak-ng it builds. The links may move; the licence's duty does not.
- [ ] Test on a phone: import, edit, play, export, and About and Licences.
