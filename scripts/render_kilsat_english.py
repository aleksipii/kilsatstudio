#!/usr/bin/env python3
"""Render the three unpublished English Kilsat episodes with installed Kokoro.

Uses the existing production exporter, sample clock and independently checked
media receipts. Downloads nothing and grants no artistic or publication approval.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import time

from render_kilsat_season import (MAX_JSON, PROFILE, UNMEASURED, file_hash,
                                  read_json, run_bounded, season_path,
                                  validate_episode, write_json)

REPO = Path(__file__).resolve().parents[1]
VOICES = {"AINO": "af_heart", "RANE": "am_michael"}
PROFILE_KEYS = {"python", "runtime_script", "model_config", "model_weights",
                "voices_directory", "cache_directory", "model_version",
                "timeout_seconds"}


def safe_path(path: Path) -> Path:
    """Refuse symbolic directory traversal before resolving a source/output path."""
    path = path.absolute()
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise ValueError(f"symbolic source/output path is unsupported: {path}")
    return path


def expected_dialogue(project: dict) -> list[dict]:
    source = json.loads(project["source"])
    if source.get("project", {}).get("captions", {}).get("enabled") is not True:
        raise ValueError("English episodes must keep their dialogue captions enabled")
    characters = source.get("characters", {})
    if set(characters) != set(VOICES):
        raise ValueError("English episodes require exactly AINO and RANE")
    for actor, preset in VOICES.items():
        voice = characters[actor].get("voice", {})
        if (voice.get("provider"), voice.get("language"), voice.get("preset")) != ("kokoro", "en-us", preset):
            raise ValueError(f"unexpected English voice binding for {actor}")
    dialogue = []
    for scene in source.get("scenes", []):
        lines = [event for event in scene.get("events", []) if event.get("type") == "speak"]
        if len(lines) > 1:
            raise ValueError("measured retiming requires at most one dialogue line per scene")
        for line in lines:
            actor = "AINO" if len(dialogue) % 2 == 0 else "RANE"
            if (line.get("actor") != actor or line.get("audio") is not None
                    or not isinstance(line.get("text"), str) or not line["text"].strip()):
                raise ValueError("English episodes require eight alternating synthesized lines")
            dialogue.append({"scene": scene["id"], "actor": actor,
                             "language": "en-us", "preset": VOICES[actor]})
    if len(dialogue) != 8:
        raise ValueError("English episode must have exactly eight spoken lines")
    return dialogue


def load_manifest(root: Path) -> tuple[dict, dict]:
    root = safe_path(root)
    manifest = read_json(root / "manifest.json")
    if (manifest.get("schema_version") != 1 or manifest.get("language") != "en-us"
            or manifest.get("profile") != PROFILE
            or manifest.get("publishable") is not False
            or manifest.get("acceptance") != UNMEASURED):
        raise ValueError("expected the unpublished English Full HD source profile")
    episodes = manifest.get("episodes", [])
    if [episode.get("number") for episode in episodes] != [1, 2, 3]:
        raise ValueError("expected three ordered English episodes")
    assets = manifest.get("asset_sha256", {})
    if not isinstance(assets, dict) or not assets:
        raise ValueError("English source asset inventory is missing")
    for name, digest in assets.items():
        if file_hash(season_path(root, name), 32 * MAX_JSON) != digest:
            raise ValueError(f"English source asset changed: {name}")
    dialogue = {}
    for episode in episodes:
        name = f"ep{episode['number']:02d}.kilsat"
        if episode.get("project") != name:
            raise ValueError("English episode number differs from its project filename")
        path = season_path(root, name)
        if file_hash(path, MAX_JSON) != episode.get("project_sha256"):
            raise ValueError(f"English project changed: {name}")
        project = read_json(path)
        source = project.get("source")
        if (not isinstance(source, str)
                or hashlib.sha256(source.encode()).hexdigest() != episode.get("source_sha256")):
            raise ValueError(f"English screenplay changed: {name}")
        for asset in project.get("assets", []):
            if asset.get("path") not in assets:
                raise ValueError("English project uses an asset outside its source inventory")
        dialogue[name] = expected_dialogue(project)
    return manifest, dialogue


def input_identity(season: Path, profile: Path, exporter: Path,
                   ffmpeg: Path, ffprobe: Path) -> dict:
    config = read_json(profile)
    if set(config) != PROFILE_KEYS:
        raise ValueError("Kokoro profile has missing or unknown fields")
    if (not isinstance(config["model_version"], str) or not config["model_version"].strip()
            or type(config["timeout_seconds"]) is not int
            or not 1 <= config["timeout_seconds"] <= 600):
        raise ValueError("Kokoro profile needs an explicit model/environment identity and bounded timeout")
    bound = {}
    for key in ("python", "runtime_script", "model_config", "model_weights"):
        path = Path(config[key])
        if not path.is_absolute():
            raise ValueError("Kokoro profile paths must be absolute")
        # A managed venv's Python is normally a symlink. Hash its actual file,
        # while preserving the invocation path in the unchanged profile.
        actual = path.resolve(strict=True)
        if key == "python" and not os.access(path, os.X_OK):
            raise ValueError("Kokoro Python must be executable")
        bound[key] = {"path": str(path), "resolved_path": str(actual), "sha256": file_hash(actual)}
    for key in ("voices_directory", "cache_directory"):
        if not Path(config[key]).is_absolute():
            raise ValueError("Kokoro profile directories must be absolute")
    voice_files = {}
    for actor, preset in VOICES.items():
        path = Path(config["voices_directory"]) / f"{preset}.pt"
        voice_files[actor] = {"preset": preset, "path": str(path), "sha256": file_hash(path)}
    tools = {}
    for name, path in (("exporter", exporter), ("ffmpeg", ffmpeg), ("ffprobe", ffprobe)):
        if not path.is_absolute() or not os.access(path, os.X_OK):
            raise ValueError("tools must be explicit absolute executable paths")
        actual = path.resolve(strict=True)
        tools[name] = {"path": str(actual), "sha256": file_hash(actual)}
    return {"schema_version": 1, "manifest_sha256": file_hash(season / "manifest.json", MAX_JSON),
            "language": "en-us", "voices": VOICES, "profile_sha256": file_hash(profile, MAX_JSON),
            "speech_files": bound, "voice_files": voice_files, "tools": tools,
            "model_environment_identity": config["model_version"],
            "dependency_closure_verified": False, "publication_approved": False}


def verify_dialogue(pipeline: dict, expected: list[dict]):
    actual = pipeline.get("dialogue_audio", [])
    if len(actual) != len(expected):
        raise ValueError("rendered English dialogue count differs from the screenplay")
    for observed, wanted in zip(actual, expected):
        if (any(observed.get(key) != value for key, value in wanted.items())
                or observed.get("imported") is not False
                or observed.get("sample_rate") != 24_000
                or type(observed.get("sample_count")) is not int
                or not 0 < observed["sample_count"] <= 24_000 * 60
                or not re.fullmatch(r"[0-9a-f]{64}", str(observed.get("pcm_sha256", "")))):
            raise ValueError("rendered dialogue language, voice or measured PCM provenance differs")


def render(season: Path, exporter: Path, profile: Path, ffmpeg: Path,
           ffprobe: Path, output: Path) -> dict:
    season = safe_path(season)
    output = safe_path(output)
    if output.exists() or output.is_relative_to(season):
        raise ValueError("select a new output directory outside the English source package")
    manifest, dialogue = load_manifest(season)
    identity = input_identity(season, profile, exporter, ffmpeg, ffprobe)
    output.mkdir(parents=True)
    logs = output / "logs"
    logs.mkdir()
    write_json(output / "render-inputs.json", identity)
    started = time.monotonic()
    report = {"schema_version": 1, "status": "failed", "language": "en-us",
              "voices": VOICES, "profile": PROFILE, "publication_approved": False,
              "acceptance": UNMEASURED, "episodes": [],
              "manifest_sha256": identity["manifest_sha256"]}
    try:
        for episode in manifest["episodes"]:
            name = episode["project"]
            stem = Path(name).stem
            directory = output / stem
            measurement_path = output / f"{stem}-measurement.json"
            command = [sys.executable, str(REPO / "scripts/benchmark_animation_pipeline.py"),
                       "--exporter", identity["tools"]["exporter"]["path"],
                       "--project", str(season / name), "--kokoro-config", str(profile),
                       "--ffmpeg", identity["tools"]["ffmpeg"]["path"],
                       "--ffprobe", identity["tools"]["ffprobe"]["path"],
                       "--output", str(directory), "--report", str(measurement_path),
                       "--retime-dialogue"]
            run_bounded(command, logs, f"{stem}-pipeline", 330)
            measured = read_json(measurement_path)
            if (measured.get("status") != "completed"
                    or measured.get("verdict", {}).get("media_valid") is not True
                    or measured.get("verdict", {}).get("full_hd_24_30_to_60_seconds") is not True):
                raise ValueError(f"English episode benchmark did not validate: {stem}")
            validated = validate_episode(directory, 1920, 1080,
                                         Path(identity["tools"]["ffprobe"]["path"]), logs,
                                         f"{stem}-independent-probe")
            verify_dialogue(validated["pipeline"], dialogue[name])
            # Detect source/profile/model/voice changes before publishing receipts.
            load_manifest(season)
            if input_identity(season, profile, exporter, ffmpeg, ffprobe) != identity:
                raise ValueError("English source, speech files or tools changed during rendering")
            receipt = {"schema_version": 1, "status": "completed", "language": "en-us",
                       "episode": episode["number"], "title": episode["title"],
                       "project_sha256": episode["project_sha256"], "voices": VOICES,
                       "publication_approved": False, "acceptance": UNMEASURED,
                       "dependency_closure_verified": False,
                       "elapsed_seconds": measured["elapsed_seconds"],
                       "measurement_sha256": file_hash(measurement_path, MAX_JSON),
                       **{key: value for key, value in validated.items() if key != "pipeline"}}
            write_json(directory / "render-receipt.json", receipt)
            report["episodes"].append(receipt)
            print(json.dumps({"episode": episode["number"], "status": "completed",
                              "observed": receipt["observed"]}), flush=True)
        report["status"] = "completed"
    except (OSError, ValueError, KeyError, TypeError, TimeoutError) as error:
        report["error"] = str(error)
    report["elapsed_seconds"] = time.monotonic() - started
    write_json(output / "english-render-report.json", report)
    return report


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--season", type=Path, default=REPO / "examples/kilsat-english")
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--kokoro-config", type=Path, required=True)
    parser.add_argument("--ffmpeg", type=Path, required=True)
    parser.add_argument("--ffprobe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        report = render(args.season, args.exporter, args.kokoro_config,
                        args.ffmpeg, args.ffprobe, args.output)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"English episode rendering failed: {error}\n")
    print(json.dumps({"status": report["status"], "episodes": len(report["episodes"]),
                      "report": str(args.output / "english-render-report.json"),
                      "error": report.get("error")}), flush=True)
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
