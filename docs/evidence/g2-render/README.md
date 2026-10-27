# G2 reference images (A08, KS-003c)

Scene: [`examples/g2-minimal-scene/scene.kilsat`](../../../examples/g2-minimal-scene/README.md)
(SHA-256 `bf58fc78afc7de8fc95d5ab040570b16009986338272ddfae7ed841f0982e175`).
Base commit `60c7310` plus this branch. No phone yet (waits for KS-003a `grip`).

| File | Frame | Shot |
| --- | --- | --- |
| `g2-wide-f024.png` | 24 (1.0 s) | wide, zoom 1 |
| `g2-close-f072.png` | 72 (3.0 s) | close-up, zoom 2.4 |

## Hashes

SVG, asserted by `reference_svgs_are_deterministic_and_match_recorded_hashes`
(`render_svg_with_art` at 960x540, no mouths, no images):

| Shot | SHA-256 |
| --- | --- |
| wide (1.0 s) | `02bf7cab570d725a4ada591d8c6c20b73cb210b2c43ba96a38a0ca35b05f2004` |
| close (3.0 s) | `64bddae7203a1b062959678091f865e1733223a090832e8d59f940371f258449` |

PNG (export pipeline, informational; not asserted by a test):

| File | Size | SHA-256 |
| --- | --- | --- |
| export `frame_000024.png` | 1920x1080 | `85e13144c14393c48e5b78e4e7370d49aefbd66ebbb54bb060f51335814ed45c` |
| export `frame_000072.png` | 1920x1080 | `d6d4c9def910e060b46edfd71e76ed6656cead1d4b35902acc3b9263e390dc1c` |
| `g2-wide-f024.png` (committed) | 960x540 | `3c03a238fe88d0a93c0d97237fc9090f0d272cbdb13bd5b3893e0a2a57f8f66e` |
| `g2-close-f072.png` (committed) | 960x540 | `c9d09435e83128bf833355cc4ccc874feb4fe35ebf2be53785bd0127ed27a8ed` |

## Renderer and platform

- Renderer: CPU only. `motion-export` example `render_scene` (PNG mode):
  shared evaluator → `render_svg_with_art` → resvg 0.45.1 / tiny-skia 0.11.4.
  The committed PNGs are those frames downscaled to 960x540 with macOS `sips -Z 960`.
- No GPU render of this scene was made; no CPU/GPU parity is claimed
  (see [render-contract.md § CPU and GPU](../../render-contract.md#cpu-and-gpu)).
- Machine: Apple Silicon (arm64), macOS 27.0.1 (26A434), rustc 1.99.1.
- Input audio: a generated 4 s silent 24 kHz mono WAV and a manifest with no
  dialogue (export requires both; the scene has no speech).

Reproduce:

```sh
python3 -I -c "import wave; w=wave.open('silence.wav','wb'); w.setnchannels(1); w.setsampwidth(2); w.setframerate(24000); w.writeframes(b'\0\0'*96000); w.close()"
echo '{"schema_version":1,"scene_id":"g2_min","sample_rate":24000,"channels":1,"total_frames":96000,"dialogue":[]}' > manifest.json
cargo run -p motion-export --example render_scene -- \
  examples/g2-minimal-scene/scene.kilsat 0 silence.wav manifest.json OUT_DIR png
sips -Z 960 OUT_DIR/frame_000024.png --out g2-wide-f024.png
sips -Z 960 OUT_DIR/frame_000072.png --out g2-close-f072.png
```

What the images show (by eye, not a perceptual review): both figures stand on the
pavement with feet visible in the wide shot; the foreground pole draws over the
background shelter; the close-up keeps VEIKKO's face and raised right hand inside
the frame. The framing and depth claims rest on the tests, not on these images.
