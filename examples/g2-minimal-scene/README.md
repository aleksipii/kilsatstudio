# G2 minimal scene (KS-003c)

`scene.kilsat` is the A08 test scene for gate G2: two stick figures (VEIKKO and
KIPINA) on the night bus-stop background, a wide shot and a hard cut to a
close-up of VEIKKO's face and raised right hand. 1920x1080, 24 fps, 4 s, silent.

| Time | Camera | Shot |
| --- | --- | --- |
| 0–1.99 s | x 0, y 0, zoom 1 | wide: both figures, feet on the pavement |
| 2.0–4 s | x −285, y −108, zoom 2.4 | close-up: face + right hand |

The cut keys are 10 ms apart (1.99 s → 2.0 s), so no 24 fps frame samples the
transition: frames 0–47 are wide, 48–95 close (tested).

**The phone is not in this scene yet.** It waits for A05's `grip` binding
(KS-003a). The rigs come from `examples/stick-studio/characters/*.kilsat`, which
do not contain the `heldprop_*` parts baked into `stick-studio.kilsat`. VEIKKO's
right arm is already raised to the face so that the phone can be bound to
`hand_r` without re-framing.

Sources (read-only, unchanged geometry): character rigs from
`examples/stick-studio/characters/veikko.kilsat` and `kipina.kilsat` (voice
metadata removed), background layers from
`examples/stick-studio/backgrounds/pysakki.kilsat` (scene ID and layer ID prefix
renamed), plus one foreground pole (`g2_fg_pole`) in front of the figures.
Regenerate byte for byte from the repository root:

```sh
python3 -I examples/g2-minimal-scene/build_scene.py .
```

Tests: `crates/motion-render/tests/g2_minimal_scene.rs`. Reference images and
hashes: [`docs/evidence/g2-render/`](../../docs/evidence/g2-render/README.md).
Contract: [`docs/render-contract.md`](../../docs/render-contract.md).
