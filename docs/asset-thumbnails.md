# Image asset previews

Open **Artwork / layers** to browse image assets. A native texture thumbnail appears beside each asset's **Place foreground** and **Place background** controls. Hover the asset name to inspect its project path. Previews preserve the image aspect ratio inside a 64-point square.

The inventory uses a bounded, virtualized scroll area so a large asset list does not construct every image row each frame. It reuses textures already decoded by the resource worker; browsing performs no filesystem reads or image decoding on the UI thread.

**No preview** means a texture is pending or unavailable. It does not confirm successful loading. Resource loading errors remain authoritative. Placement remains available, including for assets whose preview is unavailable; missing artwork keeps the existing canvas fallback behavior.

Rendering the inventory changes no project or artwork state. Placement uses the existing artwork transaction and Undo/Redo controls. Full asset import/removal history and unified project history remain open work; thumbnails do not implement them.

Headless tests cover landscape/portrait aspect fitting, native texture rendering, unavailable preview labeling, and document/history preservation when the inventory is rendered without user interaction. These do not replace native macOS visual acceptance.
