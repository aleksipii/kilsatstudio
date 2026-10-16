# Selected-scene CPU compositor

The native selected-scene preview can display a complete frame composed by the
same SVG/resvg renderer used for PNG export. This includes camera transforms,
art planes, timed visibility, opacity, rect/ellipse/polygon masks, multiply/screen
blending, animated bones and speech mouth overrides. It is a **CPU compositor**;
uploading its output into an egui texture does not establish GPU rendering parity.

`motion_export::ScenePreview` validates an immutable source/art/manifest snapshot
once without retaining or synthesizing audio. Manifest identity, 24 kHz sample
clock, clip bounds, ordered half-open viseme intervals, overlap and provenance are
checked before frame requests. The UI may provide its already evaluated per-frame
mouth snapshot for selected-line playback without allocating full-scene PCM or
inventing a manifest. Overrides require at most 256 unique known actors. With
neither a prepared manifest nor a mouth override, mouth shapes rest.
Composition is bounded to a 1024-pixel longest side and the existing source/art
resource limits. Samples use the scene-relative audio clock, including the final
scene endpoint for paused editing.

`native_compositor::Worker` owns a single persistent thread, a single active job,
one replaceable pending frame and one replaceable result. Every source, scene,
art, image-resource or manifest change must increment the caller's generation.
Time changes reuse the snapshot. Pointer identity also prevents accidental stale
snapshot reuse. Generation changes cancel active work; results carry generation,
scene and sample identity for the UI to reject stale completions. Worker Drop
cancels and wakes without joining raster work on the event loop. No files, model
assets, inference or external commands are accessed by this lane.

Automated acceptance compares selected-scene pixels against PNG export at several
speech boundaries and checks scaling, sample/size bounds, cancellation, stale
manifest rejection, queue replacement and snapshot identity. Actual interactive
Mac frame rate, device synchronization, display colour and mask/blend visual
acceptance still require physical Mac testing. Full GPU/native parity remains open.
