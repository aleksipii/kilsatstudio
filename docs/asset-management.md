# Local project assets

The Project asset manager stores image, audio and other references in the `.kilsat`
project document, alongside the exact MotionScript source. Inventory does not imply
an image is rendered or audio is placed on the timeline; those uses need explicit
scene references and supported loaders.

## Library operations

`motion_project::import_asset(project_path, &mut document, source, id, kind)` is
an explicit local copy. Save the project first so its parent establishes the asset
root. Each regular source file is limited to 256 MiB, including a bounded copy if
it grows during reading. Symlink sources and a symlink `resources` directory are
rejected. Generated portable names retain a short alphanumeric extension; users
control the inventory identifier rather than destination paths.

Copies are written to exclusive temporary files, synced, then published through
an exclusive hard link in the same directory. Existing resources are never
replaced, including name collisions. A filesystem without hard-link support
returns a recoverable error; the caller can choose a different project location.
Failed operations leave inventory and existing source/resource files unchanged.
The project is not automatically saved by these helpers: save source and updated
inventory together using `save_atomic`. An unsuccessful project save may leave an
unused imported copy, which can be relinked later.

`relink_asset(project_path, &mut document, id, relative_path)` changes inventory
metadata only. The target must exist, be a regular file and resolve under the
canonical project directory using the same resolver as asset loading. Portable
relative paths reject absolute names, parent traversal and platform-specific path
separators. A symlink escaping the root is rejected. Relink never moves or copies
the file.

`remove_asset(&mut document, id)` removes only the reference. It never deletes
an imported copy or an original source. Missing identifiers produce errors.
All operations validate the complete candidate document before replacing live
inventory, including unique IDs and document size/count limits.

Like the existing path resolver, these path-based operations assume no concurrent
hostile changes to the project filesystem. They do not claim filesystem sandboxing
against another process replacing directories during an operation. Asset imports
perform local I/O only, do not decode untrusted media, and never download model or
voice files. Desktop integrations must run the potentially large copy off the UI
thread and apply its result only to the document snapshot that requested it.
