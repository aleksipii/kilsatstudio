# Koeta Studio for iPad

Native SwiftUI app for iPad (iPadOS 16 or newer, every iPad from 2017 onwards:
iPad, iPad mini, iPad Air and iPad Pro). It uses the same Rust animation core as
the Mac app through `crates/motion-ffi`, so a scene looks the same on iPad as in
the Mac preview and export.

## What it does

Phase 1:

- Open MotionScript (`.yaml`) or a Koeta project (`.kilsat`) from the Files app,
  or one of the bundled examples.
- Scene list, play/pause, frame stepping, a timeline slider, and scrubbing by
  dragging across the picture with a finger or Apple Pencil. Tap the picture to
  play or pause. A hardware keyboard works too: space, ← and →, ⌘O.
- Works in portrait and landscape, Split View, Slide Over and Stage Manager.

Phase 2 (images and audio):

- **Open a folder** (⇧⌘O) to give the app access to a project's files. A file
  picked on its own only grants access to that file, so a project with images
  or audio files should be opened through its folder. With several project
  files in the folder, the app asks which one to open. The app's own folder
  also shows up in Files as *Koeta Studio*, so whole project folders can be
  copied onto the iPad.
- **Image assets** of a `.kilsat` project load as on the Mac (PNG or JPEG
  inside the project folder, 128 MiB decoded at most). A missing or unreadable
  image stops opening with a message that names the file.
- **Audio**: each scene's dialogue and audio cues play from imported WAV files
  (24 kHz mono PCM16, the same rule as the Mac), mixed by the same code and on
  the same 24 kHz clock as export. While audio plays, the audio position picks
  the frame, so picture and sound stay together. Scenes up to 10 minutes long.
- **Mouths** follow the same scene manifest as export. Without a manual or
  forced alignment the mouth timing is estimated from the line's text, as in a
  model-free Mac export, not measured from the audio.
- **No speech synthesis on iPad.** If a line has no `audio:` file or the file
  is missing, the scene plays silently and a note under the controls lists the
  lines. Nothing is generated in their place.

Not yet included: editing, posing with Apple Pencil, export, and screen-video
cues.

## Building

Requires Xcode, [XcodeGen](https://github.com/yonaskolb/XcodeGen) and the Rust
targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build_ipad_core.sh release           # Rust core -> Build/KoetaCore.xcframework
cd apps/studio-ipad && xcodegen generate     # -> KoetaStudio.xcodeproj
open KoetaStudio.xcodeproj
```

To run on your own iPad, choose your Apple ID team under Signing & Capabilities
(a free Apple ID works for personal testing; TestFlight and the App Store need a
paid Apple Developer account). The simulator build is arm64 only, so build it
on an Apple Silicon Mac.

For simulator smoke tests, the launch arguments `--open-example NAME` (a bundled
example such as `kilsat-season/ep02.kilsat`, or a path inside the app's Files
folder), `--scene N` (zero-based) and `--play` open a file, select a scene and
start playback once its audio is ready.
