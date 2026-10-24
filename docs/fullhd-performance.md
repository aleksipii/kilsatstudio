# Explicit streaming Full HD export

Implementation checkpoint; Rust/encoder validation is pending parent integration.

`StreamingProfile::full_hd_24()` explicitly permits up to1920×1080,24fps,
60seconds/1440frames and120seconds of retained source audio. The legacy PNG/SVG
interchange exporters retain their cumulative work bounds. Streaming composes one
frame at a time with the same scene/art/caption/crossfade renderer and pipes raw
RGBA to a caller-selected FFmpeg. It retains audio.wav,sequence.mp4,sequence.json;
it does not retain a PNG sequence. Per-frame geometry/asset validation, a256MiB
working-set estimate, cooperative cancellation, encoder watchdog, deadline and
output/log-size caps remain in force. These estimates are not a hard OS heap cap.

The default export-only deadline is300seconds and output cap512MiB; explicit
configuration is bounded to1..3600seconds and32..1024MiB. This export deadline is
not an end-to-end five-minute result. Whole-pipeline timing must also include
project loading, voice/model work, cache verification, audio preparation and
final media validation. Physical Mac performance and artistic quality are not
established by accepting a profile or a synthetic fixture.

Explicit review size is raster scaling that preserves authored world coordinates,
art/cameras, aspect ratio and frame rate. It is marked review=true; a960×540
review cannot establish1920×1080 performance or quality.

The process watchdog can interrupt a blocked raw-frame pipe by killing/reaping
FFmpeg. Scene rasterization is cooperative between bounded frames, not forcibly
preemptible mid-frame. Output is prepared in a sibling staging directory and
published to an atomically reserved new destination only after encoder confirms
all expected frames. Existing destinations are never intentionally replaced.
