# Production animation integration

Work branch: feat/production-animation-pipeline, based on main
2a0168a5d3065157033e30a8abc8947a5cc9482c.

PR #89 head 64d7293b09602c84a12254500554b2a00f2b9d7f is integrated in this
work branch: rounded custom-rig shapes and outlines. Its original CI passed;
combined-source CI and artistic review are still required.

## Implementation ownership

- direction_orchestrator: measured-audio direction worker, acting, reactions and camera.
- finnish_speech: offline Finnish providers and annotated lip-sync measurement.
- continuous_camera: explicit local tracking, recording and reviewed head keys.
- fullhd_performance: bounded streaming Full HD export and end-to-end benchmark.
- season_visuals: 31 original Finnish episodes, assets and resumable production.
- gpu_production: outlines/lights/animated props and measured parity fixtures.
- Parent integrator: schemas, desktop integration, release/Mac tooling, tests,
  GitHub checkpoints and truthful acceptance evidence.

## Acceptance boundaries

Implementation is in progress. No new subsystem is accepted merely because an
agent or interface exists. Physical camera/GUI/audio behaviour and a physical
Mac <=300-second whole-pipeline measurement require real evidence. Apple system
voices are for private preview only; final season audio needs separately
permitted local speech or cleared imported recordings. The original missing
season is not being represented as recovered; new stories are authored here.
The Kilsat prop is illustrative until a real approved capture is supplied.

Production Developer ID signing/notarization requires an authorized Apple
identity and signing environment. Final distribution, asset and dependency
rights are not granted by CI, generated inventories or ad-hoc signing.
