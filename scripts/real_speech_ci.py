#!/usr/bin/env python3
"""Helpers for the opt-in real-speech reference workflow (stdlib only).

Used by .github/workflows/real-speech-reference.yml. This module never downloads
anything and never runs inference; downloads are done by
services/kokoro-runtime/setup_kokoro.py and synthesis by
scripts/night_room_real_speech.py.

  manifest    derive a setup manifest (pinned model + selected pinned voices)
              and print its content key for actions/cache
  verify      re-hash local assets against a manifest (used on cache hits)
  provenance  write provenance.json for the produced artifact
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import sys
import wave

VOICE_NAME = re.compile(r"[ab][fm]_[a-z]+")
LANGUAGE_PREFIX = {"en-us": "a", "en-gb": "b"}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_key(manifest):
    return hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()


def derive_manifest(base, catalog, voices, language):
    """Base manifest plus the selected voices, each pinned by the catalog."""
    if catalog.get("schema_version") != 1:
        raise ValueError("unsupported voice catalog schema")
    if (catalog.get("model_repository"), catalog.get("model_revision")) != (
            base.get("model_repository"), base.get("model_revision")):
        raise ValueError("voice catalog and setup manifest pin different model revisions")
    if language not in LANGUAGE_PREFIX:
        raise ValueError(f"unsupported language {language!r}")
    pinned = {entry["name"]: entry for entry in catalog["voices"]}
    files = [dict(item) for item in base["files"]]
    by_path = {item["path"]: item for item in files}
    for voice in voices:
        if not isinstance(voice, str) or not VOICE_NAME.fullmatch(voice):
            raise ValueError(f"invalid voice name {voice!r}")
        if voice not in pinned:
            raise ValueError(f"voice {voice!r} is not pinned in the voice catalog")
        if not voice.startswith(LANGUAGE_PREFIX[language]):
            raise ValueError(f"voice {voice!r} does not match {language}")
        entry = pinned[voice]
        expected_url = ("https://huggingface.co/" + base["model_repository"] + "/resolve/"
                        + base["model_revision"] + "/voices/" + voice + ".pt")
        if entry["path"] != f"voices/{voice}.pt" or entry["url"] != expected_url:
            raise ValueError(f"voice {voice!r} catalog entry does not match the pinned revision")
        item = {k: entry[k] for k in ("path", "size_bytes", "sha256", "url")}
        existing = by_path.get(item["path"])
        if existing is not None:
            if (existing["sha256"], existing["size_bytes"]) != (item["sha256"], item["size_bytes"]):
                raise ValueError(f"voice {voice!r} hash differs between manifest and catalog")
            continue
        files.append(item)
        by_path[item["path"]] = item
    derived = dict(base)
    derived["files"] = sorted(files, key=lambda item: item["path"])
    derived["notes"] = list(base.get("notes", [])) + [
        "Derived by scripts/real_speech_ci.py for the real-speech reference workflow; "
        "extra voices are pinned by services/kokoro-runtime/english-voices.json."]
    return derived


def verify_assets(manifest, assets):
    """Return [{path, size_bytes, sha256}] or raise listing every mismatch."""
    assets = Path(assets)
    problems, verified = [], []
    for item in manifest["files"]:
        relative = PurePosixPath(item["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"unsafe manifest path {item['path']!r}")
        path = assets / relative
        if not path.is_file():
            problems.append(f"{item['path']}: missing")
            continue
        size, digest = path.stat().st_size, sha256_file(path)
        if size != item["size_bytes"] or digest != item["sha256"]:
            problems.append(f"{item['path']}: expected {item['size_bytes']} bytes sha256 "
                            f"{item['sha256']}, found {size} bytes sha256 {digest}")
            continue
        verified.append({"path": item["path"], "size_bytes": size, "sha256": digest})
    if problems:
        raise ValueError("asset verification failed:\n  " + "\n  ".join(problems))
    return verified


def wav_facts(path):
    with wave.open(str(path)) as stream:
        facts = {"channels": stream.getnchannels(), "sample_rate": stream.getframerate(),
                 "sample_width_bytes": stream.getsampwidth(), "frames": stream.getnframes()}
    if (facts["channels"], facts["sample_rate"], facts["sample_width_bytes"]) != (1, 24000, 2) \
            or facts["frames"] <= 0:
        raise ValueError(f"{path}: expected non-empty 24 kHz mono PCM16 WAV, found {facts}")
    return facts


def parse_pairs(values, convert=str):
    pairs = {}
    for value in values or []:
        key, sep, rest = value.partition("=")
        if not sep or not key:
            raise ValueError(f"expected NAME=VALUE, got {value!r}")
        pairs[key] = convert(rest)
    return pairs


def build_provenance(*, manifest, catalog, speech_report, wav_dir, artifact_dir,
                     assets, voices, speed, language, tool_versions, timings, cache_hit,
                     freeze_text, environ):
    if not speech_report:
        raise ValueError("speech report is empty; synthesis produced no lines")
    lines, total_audio, total_wall = [], 0.0, 0.0
    for entry in speech_report:
        wav = Path(wav_dir) / entry["audio"]
        facts = wav_facts(wav)
        seconds = facts["frames"] / facts["sample_rate"]
        total_audio += seconds
        total_wall += float(entry["wall_seconds"])
        lines.append({"audio": entry["audio"], "actor": entry["actor"], "voice": entry["voice"],
                      "text": entry["text"], "seconds": round(seconds, 4),
                      "wall_seconds": entry["wall_seconds"], "rtf": entry["rtf"],
                      "sha256": sha256_file(wav), "wav": facts,
                      "mouth_timing": entry.get("mouth_timing")})
    outputs = []
    root = Path(artifact_dir)
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        if path.name == "provenance.json":
            continue
        outputs.append({"path": path.relative_to(root).as_posix(),
                        "size_bytes": path.stat().st_size, "sha256": sha256_file(path)})
    selected = sorted(set(voices.values()))
    pinned = {entry["name"]: entry for entry in catalog["voices"]}
    return {
        "schema_version": 1,
        "kind": "night-room real Kokoro speech reference",
        "consent": {
            "mechanism": "manual workflow_dispatch with consent_download=true",
            "actor": environ.get("GITHUB_ACTOR"),
            "event": environ.get("GITHUB_EVENT_NAME"),
        },
        "source": {k.lower(): environ.get(k) for k in (
            "GITHUB_REPOSITORY", "GITHUB_SHA", "GITHUB_REF", "GITHUB_RUN_ID",
            "GITHUB_RUN_ATTEMPT", "GITHUB_WORKFLOW")},
        "runner": {
            "runner_os": environ.get("RUNNER_OS"), "runner_arch": environ.get("RUNNER_ARCH"),
            "runner_name": environ.get("RUNNER_NAME"), "image_os": environ.get("ImageOS"),
            "image_version": environ.get("ImageVersion"), "machine": platform.machine(),
            "macos_version": platform.mac_ver()[0] or None, "platform": platform.platform(),
        },
        "model": {
            "repository": manifest["model_repository"], "revision": manifest["model_revision"],
            "license": catalog["license"],
            "asset_cache_hit": cache_hit,
            "assets": verify_assets(manifest, assets),
        },
        "voices": {
            "selection": voices, "language": language, "speed": speed,
            "upstream_traits": {name: pinned[name]["upstream_gender_trait"] for name in selected},
        },
        "packages": {"setup_manifest_pins": manifest["packages"],
                     "english_model_url": manifest["english_model_url"],
                     "english_model_sha256": manifest["english_model_sha256"],
                     "installed_freeze": freeze_text.splitlines() if freeze_text else None},
        "tools": tool_versions,
        "timings_seconds": timings,
        "synthesis": {
            "backend": "services/kokoro-runtime/runtime.py protocol 1, local CPU, one process per line",
            "total_audio_seconds": round(total_audio, 4),
            "total_wall_seconds": round(total_wall, 2),
            "real_time_factor": round(total_wall / total_audio, 3) if total_audio else None,
            "rtf_definition": "wall seconds / audio seconds, including per-line process start and model load",
            "lines": lines,
        },
        "outputs": outputs,
        "limitations": [
            "Mouth timing is estimated from text, not aligned to the audio.",
            "WAV checks are structural (format, length); no listening or perceptual review was performed by the workflow.",
            "Licence fields are recorded from upstream metadata; they are not a legal review or redistribution approval.",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    m = sub.add_parser("manifest")
    m.add_argument("--base", type=Path, required=True)
    m.add_argument("--catalog", type=Path, required=True)
    m.add_argument("--voice", action="append", required=True)
    m.add_argument("--language", default="en-us")
    m.add_argument("--out", type=Path, required=True)
    v = sub.add_parser("verify")
    v.add_argument("--manifest", type=Path, required=True)
    v.add_argument("--assets", type=Path, required=True)
    p = sub.add_parser("provenance")
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--catalog", type=Path, required=True)
    p.add_argument("--speech-report", type=Path, required=True)
    p.add_argument("--wav-dir", type=Path, required=True)
    p.add_argument("--artifact-dir", type=Path, required=True)
    p.add_argument("--assets", type=Path, required=True)
    p.add_argument("--freeze", type=Path)
    p.add_argument("--voice", action="append", required=True, help="ACTOR=preset")
    p.add_argument("--speed", type=float, required=True)
    p.add_argument("--language", required=True)
    p.add_argument("--tool", action="append", help="NAME=version text")
    p.add_argument("--timing", action="append", help="NAME=seconds")
    p.add_argument("--cache-hit", choices=("true", "false", ""), default="")
    args = parser.parse_args(argv)
    try:
        if args.command == "manifest":
            derived = derive_manifest(json.loads(args.base.read_text()),
                                      json.loads(args.catalog.read_text()),
                                      args.voice, args.language)
            args.out.write_text(json.dumps(derived, indent=2) + "\n")
            print(f"key={canonical_key(derived)}")
        elif args.command == "verify":
            verified = verify_assets(json.loads(args.manifest.read_text()), args.assets)
            print(json.dumps({"verified": verified}, indent=2))
        else:
            freeze = args.freeze.read_text() if args.freeze and args.freeze.is_file() else None
            document = build_provenance(
                manifest=json.loads(args.manifest.read_text()),
                catalog=json.loads(args.catalog.read_text()),
                speech_report=json.loads(args.speech_report.read_text()),
                wav_dir=args.wav_dir, artifact_dir=args.artifact_dir, assets=args.assets,
                voices=parse_pairs(args.voice), speed=args.speed, language=args.language,
                tool_versions=parse_pairs(args.tool),
                timings=parse_pairs(args.timing, float),
                cache_hit={"true": True, "false": False}.get(args.cache_hit),
                freeze_text=freeze, environ=os.environ)
            (args.artifact_dir / "provenance.json").write_text(json.dumps(document, indent=2) + "\n")
            print(json.dumps(document["synthesis"] | {"lines": len(document["synthesis"]["lines"])}))
    except (OSError, ValueError, KeyError, json.JSONDecodeError, wave.Error) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
