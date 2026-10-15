# Kilsat project format v1

A `.kilsat` file is UTF-8 JSON with three required fields:

```json
{"format_version":1,"source":"characters: {}\nscenes: [{id: intro, duration: 2, events: []}]\n","assets":[]}
```

`source` stores the exact MotionScript YAML string, including comments and
whitespace. Saving and loading the document does not normalize this source.
Structured screenplay edits separately normalize YAML and participate in desktop
Undo/Redo. The shared `motion-script` parser validates every loaded/saved source.
Unknown fields and unsupported versions fail explicitly; there is no implicit
migration or attempt to interpret a future format.

Each optional inventory entry is an object with `id`, `path` and `kind`.
Identifiers are unique, nonblank and at most 256 bytes. `kind` is `image`, `audio`
or `other`; it describes inventory rather than validating file contents. Paths
use slash-separated relative components; absolute paths, traversal, empty
components, backslashes, drive prefixes and control characters are rejected.
Assets are external files, never embedded or downloaded. Inventory identifiers
are not automatically substituted into screenplay asset strings in version 1.

Documents are limited to four MiB, source to one MiB, and inventory to 4096
entries; each asset path is at most 4096 bytes. Desktop editing may impose its
existing smaller screenplay budget. File loads read at most the document limit
plus one byte. Missing external assets do not prevent document recovery; explicit
resolution reports the filesystem error.

`resolve_asset(project_path, asset)` resolves existing regular files relative to
the document's canonical parent directory and rejects canonical targets outside
that directory, including symlink escapes. It returns a path, not a held file
handle: concurrent filesystem replacement after resolution remains a caller
concern. Loading JSON alone never opens assets.

`save_atomic` validates and encodes before touching the destination, writes an
exclusively created temporary file in the same directory, syncs its contents and
renames it over the destination. Errors remove the temporary file and retain the
previous destination. If a platform cannot replace atomically, saving fails
rather than deleting the previous document. Parent-directory crash durability is
not guaranteed; concurrent saves use last-successful-rename behavior.

The MotionScript lifecycle APIs add/duplicate/remove scenes and add/remove
characters as complete validated source transactions. Scene duplication copies
events with a unique new scene identifier. Removing the final scene is rejected.
Character removal is rejected while any scene event still references that
character. These operations do not rewrite asset files or run speech synthesis.
