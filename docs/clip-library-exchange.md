# Cross-project clip exchange

This workstream implements a bounded exchange path for authored clips from the
original seven-bone rig. Its authoritative progress checklist is
[master-progress.md](master-progress.md); it does not close the character milestone.

## Implemented

- [x] Copy the selected clip or entire project library as versioned JSON text.
- [x] Paste and import that text into another project's clip-library panel.
- [x] Require explicit replacement of matching names; validate every resulting
  live reference and the complete destination before returning a source change.
- [x] Apply one import through the editor's existing source Undo/Redo transaction.
- [x] Bound exchange text, source and resulting screenplay to one MiB each, with
  the existing 64-clip and aggregate key budgets enforced by script validation.
- [x] Reject unknown envelope fields, unsupported versions/rigs, malformed clips,
  invalid numeric/key data and incompatible project combinations.

Open the bone clip library for an actor. Under **Share clips between projects**,
choose **Copy selected clip** or **Copy all clips**. In the destination project,
paste into the text box and choose **Import clips into this project**. Matching
names require the replacement checkbox, which resets when the pasted text is
edited. Imported clips do not automatically change actor tracks or create live
instances; use the existing apply or instance controls afterward. Replacing a
referenced definition changes its existing live instances after validation.

The portable envelope identifies `kilsat-clip-library`, version `1`, and rig
`original-seven-bone`; `clips` contains the existing authored clip definitions.
It carries no absolute paths, assets, models, voices or executable instructions.
YAML reserialization normalizes formatting and removes screenplay comments, as
with the existing clip-edit operations. The exchange uses clipboard/text without
performing disk or network I/O on the UI event loop.

## Still open

- [ ] Reusable custom-rig clips and explicit retargeting between rig definitions.
- [ ] Asset-bearing clip packages, texture redistribution and licensing review.
- [ ] Textured character parts, deformation and reusable idle authoring tools.
- [ ] Physical Mac clipboard/keyboard/accessibility acceptance and usability tests.

Software tests cover transfer into an unrelated project, name conflicts,
replacement, invalid envelopes and keys, size rejection, and preserving the
original project on failure. Test execution and CI evidence are recorded by the
integration PR and master progress, not implied by this implementation checklist.
