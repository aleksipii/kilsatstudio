# Unified project history workstream

Status: implemented bounded chronological desktop document history. This replaces
the earlier source audit/proposal of separate artwork and inventory histories.
The current contract and automated cases are documented in
[Unified project history](../unified-history.md).

## Implemented boundary

- `apps/studio-desktop/src/project_history.rs` stores exact source, project version,
  artwork and asset metadata as bounded whole-document snapshots.
- Toolbar Undo/Redo and Cmd/Ctrl+Z / Cmd/Ctrl+Shift+Z traverse accepted source,
  artwork and completed inventory transactions in chronological order.
- Focused screenplay sessions group edits; strokes commit on release. Snapshot
  capture occurs at mutation boundaries rather than on idle UI frames.
- Both stacks share a 64-state / eight-MiB budget. No-op and rejected operations
  preserve redo; successful new edits clear it. Raw invalid YAML stays recoverable.
- Restoration validates metadata before moving history and clears prepared
  speech/timing input. Undo never deletes, rewrites, downloads or moves media.
- Save preserves history. Opening a project/demo or changing imported-file root
  resets it; other text inputs retain their local editing undo behavior.
- Source, scene, actor and bone context changes discard detached editor drafts.
  Inventory workers retain baseline checks and cannot replace incompatible edits.

## Evidence and remaining acceptance

Automated tests cover interleaved source/art/inventory history, pending text before
artwork, rejected changes, exact raw text, redo branching/no-ops, combined bounds,
context reset and metadata-only worker undo retaining physical files. Integration
checks are reported in the PR and master progress; this document is not a claim
of native GUI execution.

On a real Mac, import an image, add a layer, edit screenplay/bone keys, then undo
and redo each action in order. Verify save preserves the timeline and reopening
resets it. Exercise shortcuts with screenplay, polygon and other text fields
focused, stale worker completion, missing files and memory-budget eviction.
Confirm playback and prepared timing cannot survive restoration as stale results.
Physical Mac keyboard/device acceptance and production usability remain open.
