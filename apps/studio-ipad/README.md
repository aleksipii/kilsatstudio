# Koeta Studio for iPad

Native SwiftUI app for iPad (iPadOS 16 or newer, every iPad from 2017 onwards:
iPad, iPad mini, iPad Air and iPad Pro). It uses the same Rust animation core as
the Mac app through `crates/motion-ffi`, so a scene looks the same on iPad as in
the Mac preview and export.

## Phase 1 (this version)

- Open MotionScript (`.yaml`) or a Koeta project (`.kilsat`) from the Files app,
  or one of the two bundled examples.
- Scene list, play/pause, frame stepping, a timeline slider, and scrubbing by
  dragging across the picture with a finger or Apple Pencil. Tap the picture to
  play or pause. A hardware keyboard works too: space, ← and →, ⌘O.
- Works in portrait and landscape, Split View, Slide Over and Stage Manager.

Not yet included: editing, posing with Apple Pencil, audio playback and export.
Speech synthesis (Kokoro) does not run on iPad; dialogue audio will be imported
as files. Image assets that a `.kilsat` project references from disk are not
loaded yet, so such a project shows an error instead of a wrong picture.

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

`--open-example hello-emma.yaml` as a launch argument opens a bundled example at
start, which is handy for simulator smoke tests.
