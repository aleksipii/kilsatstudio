# Three English Kilsat episodes

The English pack follows the same original characters as the Finnish season:
Aino is grounded and dry; Rane is smug, mean and remarkably generous with other
people's time and money. The episodes are English adaptations written for this
project, with English dialogue, captions and an English illustrative mileage log.

| Episode | Title | Setting | Story |
| --- | --- | --- | --- |
| 01 | The Free Commute | Parking lot | Rane keeps his commute free by moving its cost into Aino's lunch budget. |
| 02 | The No-Travel Meeting | Office | Everyone must drive to the meeting about reducing travel. |
| 03 | The Savings Bonus | Cafe | Aino saves the driving; Rane claims the bonus and hands her the coffee bill. |

The [readable scripts](../examples/kilsat-english/EPISODES.md) and editable
`ep01.kilsat`–`ep03.kilsat` projects are in `examples/kilsat-english`.
Each has eight dialogue turns, a three-second Kilsat insert and a silent reaction.
The source profile is **1920 × 1080, 24 fps, 30–60 seconds** per episode.
Source durations are drafts; actual local speech determines the final shot timing.

## Actual videos

The new [English render workflow](https://github.com/aleksipii/kilsatstudio/actions/workflows/english-episode-previews.yml)
builds the existing production exporter and renders all three episodes using
real local Kokoro speech: **Aino: af_heart**, **Rane: am_michael**, **en-US**.
The requested model, voices and language wheel are pinned and SHA-256 checked.
No model weights or inference environment are stored in Git.

The first run is pending. A successful artifact contains `videos/ep01/sequence.mp4`,
`videos/ep02/sequence.mp4` and `videos/ep03/sequence.mp4`, their PCM WAVs,
pipeline/measurement reports and independently checked render receipts.
`videos/english-render-report.json` records completed episodes and their hashes.
The source manifest's `not_synthesized`/`not_rendered` fields describe the source
package; render receipts describe what an actual run produced.

## Render again locally

Open a project in the desktop app, or build the existing headless exporter:

```sh
cargo build --locked --release -p motion-export --example production_export
```

Use an already installed Kokoro environment and an explicit exporter profile
with `python`, `runtime_script`, `model_config`, `model_weights`,
`voices_directory`, `cache_directory`, `model_version`, and `timeout_seconds`.
The selected voice directory must contain the two actual voice files.
All paths are absolute. The runner does not install or download anything.

```sh
python3 scripts/render_kilsat_english.py \
  --exporter /absolute/kilsatstudio/target/release/examples/production_export \
  --kokoro-config /absolute/english-kokoro-profile.json \
  --ffmpeg /absolute/ffmpeg \
  --ffprobe /absolute/ffprobe \
  --output /absolute/new-english-video-directory
```

The runner preserves the same preview/export evaluator and sample clock, retimes
each speaking shot from actual PCM, and verifies the decoded H.264/AAC frame
counts, resolution, duration, audio sample counts, languages and voice bindings.
Each episode has a bounded 300-second measured export attempt. Observed CI time
does not establish the complete workflow's timing on a physical Mac.

Regenerate sources deterministically with `python3 scripts/generate_kilsat_english.py`.
The existing 31 Finnish projects and their illustrative prop remain unchanged.

## Review before publication

These are review videos. Artistic/comic timing, listening and annotated lip-sync
quality, physical Mac acceptance and final model/voice/dependency/asset rights
remain open. The visible Kilsat insert is labelled **EXAMPLE** and is an original
illustration; an approved real product capture is still needed for marketing.
The workflow does not publish to YouTube or grant distribution approval.
