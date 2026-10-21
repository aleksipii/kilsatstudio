#!/usr/bin/env python3
"""Synthesize the night-room reference dialogue with the locally installed Kokoro.

Offline, local CPU only; downloads nothing. Run with the managed Kokoro venv:

  "$HOME/Library/Application Support/Kilsat Studio/Kokoro/venv/bin/python" -I \
      scripts/night_room_real_speech.py [--out DIR]

Reads the lines from examples/night-room-reference/shots.yaml, calls
services/kokoro-runtime/runtime.py once per line (the same file protocol the app
uses) and writes 24 kHz mono PCM16 WAVs to DIR/<line audio path>. The default DIR
is outside Git. Prints per-line duration, wall time and real-time factor (wall /
audio) as JSON. This is a structural run, not a listening or alignment result.
"""
import argparse
import json
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
VOICES = {"DAD": "am_michael", "STAN": "af_heart"}


def lines():
    shots = yaml.safe_load((ROOT / "examples/night-room-reference/shots.yaml").read_text())
    return [s["line"] for s in shots["shots"] if s.get("line")]


def main():
    home = Path.home() / "Library/Application Support/Kilsat Studio"
    p = argparse.ArgumentParser()
    p.add_argument("--kokoro", type=Path, default=home / "Kokoro")
    p.add_argument("--out", type=Path, default=home / "night-room-audio")
    p.add_argument("--speed", type=float, default=1.0)
    args = p.parse_args()
    assets = args.kokoro / "assets"
    report = []
    for line in lines():
        out = args.out / line["audio"]
        out.parent.mkdir(parents=True, exist_ok=True)
        voice = VOICES[line["actor"]]
        with tempfile.TemporaryDirectory() as tmp:
            request, response = Path(tmp, "req.json"), Path(tmp, "resp.json")
            request.write_text(json.dumps({
                "protocol": 1,
                "request": {"text": line["text"], "voice": {
                    "preset": voice, "language": "en-us", "speed": args.speed, "style": None}},
                "config_path": str(assets / "config.json"),
                "model_path": str(assets / "kokoro-v1_0.pth"),
                "voice_path": str(assets / "voices" / f"{voice}.pt"),
                "output_path": str(out),
            }))
            started = time.monotonic()
            subprocess.run([sys.executable, "-I", str(ROOT / "services/kokoro-runtime/runtime.py"),
                            "--request", str(request), "--response", str(response)], check=True)
            wall = time.monotonic() - started
            result = json.loads(response.read_text())
        if not result.get("ok"):
            sys.exit(f"{line['audio']}: {result}")
        with wave.open(str(out)) as w:
            seconds = w.getnframes() / w.getframerate()
        report.append({"audio": line["audio"], "actor": line["actor"], "voice": voice,
                       "text": line["text"], "seconds": round(seconds, 4),
                       "wall_seconds": round(wall, 2), "rtf": round(wall / seconds, 2),
                       "phonemes": result["phonemes"], "path": str(out)})
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
