# Unified project history

The desktop toolbar's Undo and Redo, Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z now
navigate one chronological document history. It includes screenplay transactions
(settings, events, voices, lifecycle commands and focused text sessions),
committed canvas/inspector artwork edits, and completed asset-inventory operations.
The artwork and asset panels no longer expose separate competing undo stacks.

Each snapshot contains the exact screenplay text, project format version, artwork
and asset references. Undoing an import/removal/relink restores metadata only:
physical files are never deleted, overwritten, redownloaded or moved by history.
Image/audio buffers, prepared speech, runtime configuration and alignment input
fields are not stored in snapshots. Restoring a document discards prepared audio
and timing input so stale speech cannot be presented as current.

Incomplete raw YAML is a legitimate text-history state. Restoration preserves
that text and reports validation errors instead of skipping the state. Metadata
is checked independently, and states captured as valid projects are checked
against their screenplay before either history stack moves. Rejected structured
edits do not create a history entry. New committed edits clear the redo branch;
no-op edits preserve it.

History stores at most 64 undo/redo states combined and at most eight MiB of
conservatively encoded snapshot bytes across both stacks. The current document
is owned by the editor rather than duplicated in the history. Farthest states
are evicted first. A snapshot larger than the budget cannot be retained for
undo; subsequent smaller transactions still work. History is session-local and
resets when opening a project/demo or changing the imported-file project root.
Saving preserves the current undo/redo timeline.

Snapshots are captured only at committed mutation boundaries. No full document
snapshot, serialization or artwork traversal was added to the idle frame loop.
Canvas strokes commit once on release; direct artwork control changes commit as
accepted edits. Text fields other than the screenplay retain their native local
text undo behavior. The screenplay uses the global document history.

Validation covers interleaved source/art/inventory history, raw-invalid text,
redo branching and no-ops, metadata rejection without moving stacks, combined
step/byte budgets, project context reset, a pending text session followed by an
actual artwork transaction, rejected artwork, and the real asset worker's
metadata-only undo with retained physical files. Mac keyboard/device behavior
still requires the separate macOS acceptance run.
