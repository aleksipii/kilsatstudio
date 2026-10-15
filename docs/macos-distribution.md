# macOS distribution status

`scripts/package_macos.py` stages a preview `.app`, signs it ad hoc, verifies the
signature and archives it with `ditto` on macOS. The existing macOS workflow builds
these artifacts. They are preview builds, not notarized production releases.

The bundle includes an independently authored geometric K/film icon generated
in Python as a 256×256 RGBA PNG inside a standard `ic08` ICNS container. No external
artwork, image tool, downloaded asset or added dependency is required. Packaging
unit tests inspect the ICNS lengths, PNG dimensions/CRC/decompressed data and plist
reference. Finder/Dock visual appearance must still be checked on a Mac.

LaunchServices declarations advertise `.kilsat` JSON project documents using the
exported `studio.kilsat.project` type (conforming to `public.json`) and `.yaml`/`.yml`
MotionScript documents. Both use the Editor role and Alternate rank, avoiding an
intended default-handler takeover of generic YAML. These declarations do not
implement application event delivery: opening documents by double-click, Finder
Open With or dropping onto the Dock requires native file-open event integration
and a separate macOS test. Use the application's existing Open controls meanwhile.

No model weights, voice packs, Python environment or unreviewed binary dependencies
are bundled. Exact license/notice and redistribution obligations remain a separate
release gate; see [Kokoro dependency review](kokoro-dependency-review.md).

Remaining release checks include Apple Silicon build/run and device playback,
real offline Kokoro CPU synthesis, icon presentation, Finder open-event delivery,
project round trips, user-facing release packaging and a Developer ID/notarization
decision. Ad-hoc signing is not Developer ID signing or notarization. See the
[master tracker](master-progress.md) for implementation versus acceptance status.
