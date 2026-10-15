# Actor transform keys

Select a character, then open its transform-key editor in the Inspector. Each
scene stores an independent track per character. Edit time (seconds), X/Y offsets,
rotation (radians), scale X/Y and Linear, Smoothstep or Hold interpolation. The
selected easing controls the segment following that key. Root offsets layer over
existing movement events; the rig's local animations remain intact.

Add key appends an identity key at time zero, or copies the last transform one
second later. It is disabled if that time exceeds the scene duration or the track
has 4096 keys. Remove key deletes only a draft entry. Reset key draft restores the
track loaded when this draft opened. Applying an empty list removes the track.

Edits stay detached until Apply transform keys. Apply validates the whole script,
including increasing times, finite transforms and scene bounds. Invalid drafts do
not change the document. Applying normalizes YAML and removes comments; Undo
restores the previous source. Busy jobs block application. Changing the scene,
character or document invalidates the old draft.

Preview and export use the same evaluated track. This is a list editor for root
transforms, not a curve editor or per-bone pose editor.
