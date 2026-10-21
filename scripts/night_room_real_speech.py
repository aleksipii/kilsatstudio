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
import math
import re
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
VOICES = {"DAD": "am_michael", "STAN": "af_heart"}


def installed_voices(assets):
    """List local English presets without importing inference or downloading assets."""
    return sorted(path.stem for path in (assets / "voices").glob("*.pt")
                  if path.is_file() and re.fullmatch(r"[ab][fm]_[A-Za-z0-9_]+", path.stem))


def validate_selection(assets, voices, language, speed):
    if not math.isfinite(speed) or not 0.5 <= speed <= 2:
        raise ValueError("speed must be finite and between 0.5 and 2")
    prefix = {"en-us": "a", "en-gb": "b"}[language]
    available = installed_voices(assets)
    for actor, preset in voices.items():
        if preset not in available:
            raise ValueError(f"{actor}: voice {preset!r} is not installed; use --list-voices")
        if not preset.startswith(prefix):
            raise ValueError(f"{actor}: voice {preset!r} does not match {language}")
    for name in ("config.json", "kokoro-v1_0.pth"):
        if not (assets / name).is_file():
            raise ValueError(f"missing local asset: {assets / name}")


def lines():
    import yaml
    shots = yaml.safe_load((ROOT / "examples/night-room-reference/shots.yaml").read_text())
    return [s["line"] for s in shots["shots"] if s.get("line")]


def main():
    home = Path.home() / "Library/Application Support/Kilsat Studio"
    p = argparse.ArgumentParser()
    p.add_argument("--kokoro", type=Path, default=home / "Kokoro")
    p.add_argument("--out", type=Path, default=home / "night-room-audio")
    p.add_argument("--speed", type=float, default=1.0)
    p.add_argument("--language", choices=("en-us", "en-gb"), default="en-us")
    p.add_argument("--dad-voice", default=VOICES["DAD"])
    p.add_argument("--boy-voice", default=VOICES["STAN"])
    p.add_argument("--list-voices", action="store_true",
                   help="list installed English voice assets; does not load the model")
    args = p.parse_args()
    assets = args.kokoro / "assets"
    if args.list_voices:
        print(json.dumps({"voices": installed_voices(assets),
                          "note": "Local preset files only; age, suitability and acoustic quality are unreviewed."},
                         indent=2))
        return
    voices = {"DAD": args.dad_voice, "STAN": args.boy_voice}
    try:
        validate_selection(assets, voices, args.language, args.speed)
    except ValueError as error:
        p.error(str(error))
    report = []
    for line in lines():
        out = args.out / line["audio"]
        out.parent.mkdir(parents=True, exist_ok=True)
        voice = voices[line["actor"]]
        with tempfile.TemporaryDirectory() as tmp:
            request, response = Path(tmp, "req.json"), Path(tmp, "resp.json")
            request.write_text(json.dumps({
                "protocol": 1,
                "request": {"text": line["text"], "voice": {
                    "preset": voice, "language": args.language, "speed": args.speed, "style": None}},
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
                       "text": line["text"], "language": args.language,
                       "mouth_timing": "estimated_from_text_not_audio_aligned",
                       "seconds": round(seconds, 4),
                       "wall_seconds": round(wall, 2), "rtf": round(wall / seconds, 2),
                       "phonemes": result["phonemes"], "path": str(out)})
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
