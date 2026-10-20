#!/usr/bin/env python3
"""Check authored design fixtures, not screenplay inference or rendered quality.

No third-party dependencies. A successful check means only that the proposed
contract example is internally consistent; Studio does not consume this format.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs/automation/plan.json"
PILOT = ROOT / "examples/automation/pilot-intent.json"
SOURCE = ROOT / "examples/automation/pilot-fi.txt"
SUPPORTED_LANGUAGES = ("fi-FI", "en-US", "en-GB")


class PlanError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise PlanError(message)


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def integer(value):
    return type(value) is int


def validate(manifest, intent, source):
    """Validate bounded, reviewed example data; perform no animation work."""
    require(type(manifest.get("version")) is int and manifest["version"] == 1, "unsupported plan version")
    require(manifest.get("status") == "design", "plan must remain labelled design")
    require(manifest.get("supported_languages") == list(SUPPORTED_LANGUAGES), "language registry differs")
    agents = manifest.get("agents", [])
    require(len(agents) == 10, "exactly ten agent reports required")
    require([a["id"] for a in agents] == list(range(1, 11)), "agent IDs must be 1..10")
    require(len({a["report"] for a in agents}) == 10, "duplicate agent report")
    for agent in agents:
        require(agent.get("status") == "design_complete", "agent status must describe design")
    caps = manifest.get("stage_caps_s", {})
    require(set(caps) == {"ingest", "planning", "assets", "voice", "alignment",
                         "acting", "render", "encode", "reserve"}, "missing or unknown stage")
    require(all(integer(v) and v > 0 for v in caps.values()), "positive integer stage caps required")
    require(sum(caps.values()) == 300, "serial stage caps must total 300 seconds")
    require(manifest.get("sla_measured") is False, "no measured SLA in this design")
    require(type(intent.get("version")) is int and intent["version"] == 1, "unsupported intent version")
    require(integer(intent.get("seed")) and 0 <= intent["seed"] <= 4294967295, "invalid seed")
    language = intent.get("language")
    require(language in SUPPORTED_LANGUAGES, "unsupported script language")
    require(intent.get("speech_language") == language, "speech language differs from script")
    require(intent.get("caption_language") == language, "caption language differs from script")
    duration = intent.get("duration_s")
    require(number(duration) and 30 <= duration <= 60, "duration must be 30..60 seconds")
    require(isinstance(source, bytes) and len(source) <= 65536, "source exceeds 64 KiB")
    source.decode("utf-8")
    require(intent.get("source_sha256") == hashlib.sha256(source).hexdigest(), "stale source hash")
    characters = intent.get("characters", [])
    require(1 <= len(characters) <= 2, "one or two characters required")
    actors = {c["id"] for c in characters}
    require(len(actors) == len(characters), "duplicate actor ID")
    scenes = intent.get("scenes", [])
    require(1 <= len(scenes) <= 3, "one to three story scenes required")
    scene_durations = {s["id"]: s["duration_s"] for s in scenes}
    require(len(scene_durations) == len(scenes), "duplicate scene ID")
    require(all(number(v) and v > 0 for v in scene_durations.values()), "invalid scene duration")
    require(math.isclose(sum(scene_durations.values()), duration, abs_tol=1e-9), "scene total differs")
    scene_windows = {}
    offset = 0
    for scene in scenes:
        scene_windows[scene["id"]] = (offset, offset + scene["duration_s"])
        offset += scene["duration_s"]
    beats = intent.get("beats", [])
    require(1 <= len(beats) <= 64, "one to 64 beats required")
    require(len({b["id"] for b in beats}) == len(beats), "duplicate beat ID")
    dialogue_chars = 0
    actor_spans = {}
    all_dialogue = []
    rules = manifest.get("ingestion_rule_ids", [])
    require(rules == [f"ING-{i:03}" for i in range(1, 7)], "incomplete ingestion rule registry")
    for beat in beats:
        require(beat.get("scene") in scene_durations, "unknown scene")
        start, end = beat.get("start_s"), beat.get("end_s")
        require(number(start) and number(end) and 0 <= start < end <= scene_durations[beat["scene"]],
                "beat outside scene or nonpositive interval")
        require(beat.get("kind") in ("dialogue", "action", "hold"), "unsupported beat kind")
        span = beat.get("source_span", {})
        a, b = span.get("start_byte"), span.get("end_byte")
        require(integer(a) and integer(b) and 0 <= a < b <= len(source), "invalid source span")
        try:
            excerpt = source[a:b].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise PlanError("span splits a UTF-8 character") from exc
        require(excerpt == beat.get("source_text"), "source span text mismatch")
        provenance = beat.get("provenance", {})
        require(provenance.get("rule_id") in rules and provenance.get("rule_pack") == "proposal-v1"
                and provenance.get("kind") == "authored", "missing or unknown authored provenance")
        if beat["kind"] == "dialogue":
            require(beat.get("actor") in actors, "unknown speaker")
            require(beat.get("text") == excerpt and bool(excerpt.strip()), "dialogue changed")
            dialogue_chars += len(excerpt)
            key = beat["scene"], beat["actor"]
            actor_spans.setdefault(key, []).append((start, end))
            base = scene_windows[beat["scene"]][0]
            all_dialogue.append((base + start, base + end))
        elif beat["kind"] == "action":
            require(beat.get("actor") in actors, "unknown action actor")
            require(beat.get("action") in ("look_at", "present_prop", "react", "idle"),
                    "unsupported action")
    require(dialogue_chars <= 1000, "dialogue exceeds quick profile bound")
    for spans in actor_spans.values():
        ordered = sorted(spans)
        require(all(a[1] <= b[0] for a, b in zip(ordered, ordered[1:])), "overlapping actor dialogue")
    ordered = sorted(all_dialogue)
    require(all(a[1] <= b[0] for a, b in zip(ordered, ordered[1:])), "overlapping speakers unsupported")
    shots = intent.get("shot_proposal", [])
    require(1 <= len(shots) <= 16, "one to sixteen final shots including reactions required")
    require(len({s["id"] for s in shots}) == len(shots), "duplicate shot ID")
    cursor = 0
    for shot in shots:
        require(shot.get("scene") in scene_durations, "shot references unknown story scene")
        start, end = shot.get("start_s"), shot.get("end_s")
        require(number(start) and number(end) and start == cursor and start < end <= duration,
                "shots must cover global time without gaps or overlaps")
        lo, hi = scene_windows[shot["scene"]]
        require(lo <= start < end <= hi, "shot outside referenced story scene")
        cursor = end
    require(cursor == duration, "shots must cover entire film")
    insert = intent.get("app_insert", {})
    start, end = insert.get("start_s"), insert.get("end_s")
    require(number(start) and number(end) and 0 <= start < end <= duration and end-start == 5,
            "pilot requires five-second app insert")
    app_shots = [s for s in shots if s.get("intent") == "app_insert"]
    require(len(app_shots) == 1 and app_shots[0]["start_s"] == start
            and app_shots[0]["end_s"] == end, "app insert and shot interval differ")
    require(insert.get("asset_status") == "missing_real_capture", "real app capture is still missing")
    require(intent.get("audio_status") == "missing_recording", "fixture audio is still missing")
    require(intent.get("render_status") == "not_rendered", "fixture is not rendered animation")
    return {"status": "design_fixture_valid", "agents": len(agents), "duration_s": duration,
            "shots": len(shots), "language": language,
            "animation_rendered": False, "sla_measured": False}


def check_reports(manifest):
    root = ROOT.resolve()
    for agent in manifest["agents"]:
        report = (root / agent["report"]).resolve()
        require(report.is_relative_to(root) and report.suffix == ".md", "unsafe report path")
        require(report.is_file() and report.stat().st_size > 1000, "missing or empty agent report")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        examples = [(PILOT, SOURCE), (ROOT / "examples/automation/pilot-en-intent.json",
                                     ROOT / "examples/automation/pilot-en.txt")]
        result = {"status": "design_fixtures_valid", "examples": [
            validate(manifest, json.loads(pilot.read_text(encoding="utf-8")), source.read_bytes())
            for pilot, source in examples]}
        check_reports(manifest)
    except (PlanError, ValueError, KeyError, TypeError, OSError) as exc:
        parser.exit(1, f"Design check failed: {exc}\n")
    output = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(output, encoding="utf-8")
    print(output, end="")


if __name__ == "__main__":
    main()
