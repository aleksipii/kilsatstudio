# Unified project history workstream

Status: design and source audit only. Unified history is **not implemented** by this report.
The current histories remain separate; this document does not establish acceptance.

## Current behavior verified in source

- `apps/studio-desktop/src/edit_history.rs` stores exact screenplay `String` snapshots,
  bounded to 64 retained undo/redo entries and 8 MiB, excluding current source.
  A changed source baseline resets history; a new committed edit clears redo.
- `main.rs::flush_text_edit` groups focused screenplay edits. `undo_edit` validates
  restored source against the current project art/inventory before applying it.
  Conflicting scene/character references reject restoration and restore the history position.
- `apps/studio-desktop/src/art_editor.rs` owns separate artwork Undo/Redo.
  Commit bounds undo to 24 entries / 8 MiB, retaining at least one prior document.
  `main.rs::art_inspector_ui` clears this history when source, assets or scene context changes;
  whole-project validation rolls invalid artwork back and invalidates its local history.
- Inventory has a single `main.rs::asset_undo` snapshot, without a corresponding redo.
  Its UI restores previous metadata with the current source substituted.
  `.kilsat` save clears this inventory undo slot and the artwork/inventory dirty flags.
- `asset_pending` accepts asynchronous results only while source, path, art and assets
  match its captured baseline. Rejected results leave imported physical files on disk.
- `crates/motion-project/src/lib.rs::ProjectDocument` contains source, assets and art.
  Its validation guards scene IDs, actor attachments, image asset IDs/types and active intervals.
  `crates/motion-project/src/assets.rs::remove_asset` removes metadata only and validates
  the entire candidate; it never deletes original or copied asset files.

## Proposed bounded transaction model

1. One owner stores complete `ProjectDocument` snapshots plus document identity/revision.
   Preserve source strings exactly, including comments, spacing and line endings;
   do not regenerate YAML merely to record, undo, redo or save a history entry.
2. Each validated inspector action, completed canvas gesture, grouped text edit or
   accepted inventory operation commits one before/after transaction across all domains.
   Scene rename/removal and associated art changes must validate together before commit.
3. Keep 64 total retained undo/redo entries and an explicit serialized byte budget
   covering source + assets + art in both stacks. Current document is separately bounded.
   Use checked accounting and deterministic eviction; an oversized transaction must never
   corrupt the current state or silently remove only part of a transaction.
4. New successful commits invalidate redo; no-op/rejected/stale operations preserve it.
   Prevalidate restoration before moving the cursor so failure cannot consume history.
5. History restores metadata only. Import/relink/remove and redo never delete, overwrite,
   recopy or fetch filesystem assets. Missing files yield diagnostics rather than destructive
   repair. Preserve the existing relative-path and project-root relocation restrictions.
6. Keep the last successfully saved document identity/content as the dirty baseline.
   Undo back to that baseline becomes clean; redo away becomes dirty. Save failures leave
   the baseline untouched. Saving must retain undo/redo; opening another document resets it.
7. Capture a monotonically changing revision and project identity for every worker request.
   Undo, redo, open and committed edits invalidate old requests even after an ABA edit
   returns to equal content. Stale completion must not replace state or reset dirty status.
8. Invalid in-progress YAML must remain editable and recoverable. Define a raw-source
   draft envelope alongside the last valid project; never discard text to force validation.
   Flush/group drafts before cross-domain commands, and reject incompatible commands
   without changing the text, history, imported assets or document.
9. After restoration invalidate scene/image/alignment/job caches, cancel active gestures
   and stop playback. Preserve existing busy/export/diagnostic guards until workers carry
   immutable snapshots and revision checks; do not replay side effects during redo.

## Migration sequence and exact files

1. Extend `apps/studio-desktop/src/edit_history.rs` with full-document transactions,
   byte accounting, cursor-safe validation and a saved baseline; retain source-only tests.
2. Add a single capture/validate/commit/restore boundary in
   `apps/studio-desktop/src/main.rs`; route every existing `history.record` and text flush
   through it, synchronizing `source` with `project_document.source` at that boundary.
3. Change `apps/studio-desktop/src/art_editor.rs` to return committed candidates/actions;
   remove its local stacks only after gestures and selection survive global restoration.
4. Replace `asset_undo` in `main.rs` with global transactions. Extend
   `apps/studio-desktop/src/asset_worker.rs` requests/results with revision/identity tokens.
5. Keep validation and metadata-only asset semantics in
   `crates/motion-project/src/lib.rs` and `crates/motion-project/src/assets.rs`.
   Add atomic multi-domain validation tests there before enabling coupled edits.
6. Unify shortcuts/buttons and dirty indicators in `main.rs`, including open/demo/reset
   and successful/failed save handling; remove old history fields only after integration tests.

## Required acceptance tests before marking complete

- Import image → add image layer → rename scene with layer reference → undo each step
  and redo each step; assert whole-document validity and exact original source bytes.
- Delete a layer and its inventory entry atomically; undo restores both, while standalone
  referenced-asset removal rejects without moving history. Physical files remain byte-identical.
- Interleave artwork edits, screenplay text and inventory relink; one global chronological
  undo order, redo branch invalidation, no-op preservation and no detached local history.
- Save midway, undo/redo across the saved state, fail a save, reopen another project:
  correct dirty state and no history crossing document identity.
- Complete an import after undo/redo, path change or equal-content ABA edits; reject stale
  results without losing source/art/inventory, while retaining any copied files safely.
- Exercise byte/entry limits using large artwork plus source; contiguous valid restoration,
  bounded accounting, recoverable invalid text drafts and no cache/selection/playback leakage.
