#!/usr/bin/env python3
"""Render one original Kilsat season group with real, explicitly installed speech.

Produces episode MP4s, a compilation and independently checked receipts. Review
renders do not grant speech, artwork, lip-sync, physical-Mac or publication approval.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import signal
import subprocess
import sys
import time
import wave

MAX_JSON = 4 * 1024 * 1024
MAX_MEDIA = 1024 * 1024 * 1024
PROFILE = {"fps": 24, "height": 1080, "max_episode_seconds": 60,
           "min_episode_seconds": 30, "width": 1920}
UNMEASURED = {"artistic_review": False, "audio_rights_reviewed": False,
              "real_audio_lipsync_review": False, "actual_kilsat_capture": False,
              "physical_mac_300s": False}


def file_hash(path: Path, limit=MAX_MEDIA) -> str:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError(f"expected bounded regular file: {path}")
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            size += len(chunk)
            if size > limit:
                raise ValueError(f"file grew beyond budget: {path}")
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict:
    file_hash(path, MAX_JSON)
    with path.open("rb") as source:
        data = source.read(MAX_JSON + 1)
    if len(data) > MAX_JSON:
        raise ValueError("JSON grew beyond budget")
    value = json.loads(data)
    if not isinstance(value, dict):
        raise ValueError("expected a JSON object")
    return value


def write_json(path: Path, value: dict):
    data = (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()
    if len(data) > MAX_JSON:
        raise ValueError("receipt exceeds its byte budget")
    # Receipts are published only after validation, without replacing an old one.
    with path.open("xb") as target:
        target.write(data)


def season_path(root: Path, relative: str) -> Path:
    if not isinstance(relative, str) or "\\" in relative:
        raise ValueError("invalid season-relative path")
    path = PurePosixPath(relative)
    if path.is_absolute() or not path.parts or any(p in (".", "..") for p in relative.split("/")):
        raise ValueError("season paths must stay inside the source directory")
    result = root
    for part in path.parts:
        result = result / part
        if result.is_symlink():
            raise ValueError("season source paths cannot traverse symbolic links")
    return result


def load_manifest(root: Path) -> dict:
    manifest = read_json(root / "manifest.json")
    if manifest.get("schema_version") != 1 or manifest.get("publishable") is not False:
        raise ValueError("expected an explicitly unpublished version-one season")
    if manifest.get("profile") != PROFILE or manifest.get("language") != "fi-fi":
        raise ValueError("expected the Finnish 1920x1080/24fps source profile")
    episodes = manifest.get("episodes", [])
    if [e.get("number") for e in episodes] != list(range(1, 32)):
        raise ValueError("season must contain all 31 ordered episodes")
    assets = manifest.get("asset_sha256", {})
    if not assets:
        raise ValueError("season asset hash inventory is missing")
    for relative, expected in assets.items():
        if file_hash(season_path(root, relative), 32 * MAX_JSON) != expected:
            raise ValueError(f"season asset changed: {relative}")
    for episode in episodes:
        if episode.get("project") != f"ep{episode['number']:02}.kilsat":
            raise ValueError("episode number and project filename differ")
        path = season_path(root, episode["project"])
        if file_hash(path, MAX_JSON) != episode.get("project_sha256"):
            raise ValueError(f"episode project changed: {path.name}")
        project = read_json(path)
        source = project.get("source")
        if not isinstance(source, str) or hashlib.sha256(source.encode()).hexdigest() != episode.get("source_sha256"):
            raise ValueError(f"episode screenplay hash differs: {path.name}")
        for asset in project.get("assets", []):
            if asset.get("path") not in assets:
                raise ValueError("project refers to an asset outside the manifest")
    groups = manifest.get("youtube_groups", [])
    if len(groups) != 7:
        raise ValueError("season requires six five-episode groups and the finale")
    for index, group in enumerate(groups, 1):
        expected = list(range((index - 1) * 5 + 1, min(index * 5, 31) + 1))
        if (group.get("number") != index or group.get("episodes") != expected
                or group.get("publishable") is not False):
            raise ValueError("compilation membership differs from the season")
    return manifest


def input_identity(season: Path, exporter: Path, profile: Path, ffmpeg: Path,
                   ffprobe: Path, group: int, width: int, height: int) -> dict:
    config = read_json(profile)
    speech = {}
    for key in ("executable", "model_path", "config_path"):
        path = Path(config[key])
        if not path.is_absolute():
            raise ValueError("Piper profile paths must be absolute")
        speech[key] = {"path": str(path), "sha256": file_hash(path)}
    if not str(config.get("preset", "")).startswith("piper_"):
        raise ValueError("this season requires an explicitly selected Finnish Piper voice")
    tools = {name: {"path": str(path), "sha256": file_hash(path)}
             for name, path in (("exporter", exporter), ("ffmpeg", ffmpeg), ("ffprobe", ffprobe))}
    return {"schema_version": 1, "manifest_sha256": file_hash(season / "manifest.json", MAX_JSON),
            "group": group, "width": width, "height": height, "fps": 24,
            "profile_sha256": file_hash(profile, MAX_JSON), "speech_files": speech,
            "tools": tools, "dependency_closure_verified": False,
            "environment_identity_claim": config.get("environment_identity"),
            "publication_approved": False}


def run_bounded(command: list[str], directory: Path, stem: str, timeout: float):
    stdout, stderr = directory / f"{stem}.stdout.log", directory / f"{stem}.stderr.log"
    with stdout.open("wb") as out, stderr.open("wb") as err:
        child = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                 start_new_session=(os.name == "posix"))
        started = time.monotonic()
        try:
            while child.poll() is None:
                if time.monotonic() - started > timeout:
                    raise TimeoutError(f"{stem} exceeded its {timeout:g}s deadline")
                if max(stdout.stat().st_size, stderr.stat().st_size) > MAX_JSON:
                    raise ValueError(f"{stem} exceeded its log byte budget")
                time.sleep(0.05)
        except BaseException:
            if os.name == "posix":
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                child.kill()
            child.wait()
            raise
    if max(stdout.stat().st_size, stderr.stat().st_size) > MAX_JSON:
        raise ValueError(f"{stem} exceeded its log byte budget")
    if child.returncode:
        raise ValueError(f"{stem} exited {child.returncode}: {stderr.read_text(errors='replace')[-4096:]}")
    return stdout


def inspect_media(media: dict, width: int, height: int, frames: int) -> dict:
    streams = media.get("streams", [])
    video = [s for s in streams if s.get("codec_type") == "video"]
    audio = [s for s in streams if s.get("codec_type") == "audio"]
    if len(video) != 1 or len(audio) != 1:
        raise ValueError("expected exactly one actual video stream and one audio stream")
    video, audio = video[0], audio[0]
    rate = str(video.get("avg_frame_rate", "0/1")).split("/")
    fps = float(rate[0]) / (float(rate[1]) if len(rate) == 2 else 1)
    duration = float(media["format"]["duration"])
    if not math.isfinite(duration) or not math.isfinite(fps):
        raise ValueError("nonfinite media duration or frame rate")
    if (video.get("codec_name") != "h264" or audio.get("codec_name") != "aac"
            or int(video.get("width", 0)) != width or int(video.get("height", 0)) != height
            or abs(fps - 24) > 0.0001 or int(video.get("nb_read_frames", -1)) != frames
            or int(audio.get("sample_rate", 0)) != 24_000 or int(audio.get("channels", 0)) != 1):
        raise ValueError("actual decoded frames, dimensions, rate or codecs differ from the receipt")
    if frames <= 0 or abs(duration - frames / 24) > 0.09:
        raise ValueError("media duration differs from its frame clock")
    # Both tracks must cover the intended length; a long video cannot hide short audio.
    for stream in (video, audio):
        seconds = float(stream.get("duration", duration))
        if not math.isfinite(seconds) or abs(seconds - frames / 24) > 0.09:
            raise ValueError("audio/video track duration differs from the frame clock")
        start = float(stream.get("start_time", 0))
        if not math.isfinite(start) or abs(start) > 1 / 24_000:
            raise ValueError("audio/video tracks must start on the same zero sample")
    return {"frame_count": frames, "duration_seconds": duration,
            "width": width, "height": height, "fps": 24, "audio_sample_rate": 24_000}


def probe(ffprobe: Path, media: Path, directory: Path, stem: str) -> dict:
    stdout = run_bounded([str(ffprobe), "-v", "error", "-count_frames", "-show_streams",
                          "-show_format", "-of", "json", str(media)], directory, stem, 120)
    return read_json(stdout)


def validate_episode(directory: Path, width: int, height: int, ffprobe: Path,
                     logs: Path, stem: str) -> dict:
    pipeline = read_json(directory / "pipeline-report.json")
    streaming = pipeline.get("streaming", {})
    if pipeline.get("status") != "completed" or pipeline.get("schema_version") != 1:
        raise ValueError("exporter did not complete this episode")
    frames = int(streaming["frame_count"])
    samples = int(streaming["total_samples"])
    if (streaming.get("width"), streaming.get("height"), streaming.get("fps")) != (width, height, 24):
        raise ValueError("exporter profile differs from the requested render")
    if not 720 <= frames <= 1440 or not 0 <= frames * 1000 - samples < 1000:
        raise ValueError("episode is outside 30..60s or differs from the shared sample clock")
    media = probe(ffprobe, directory / "sequence.mp4", logs, stem)
    observed = inspect_media(media, width, height, frames)
    with wave.open(str(directory / "audio.wav"), "rb") as audio:
        if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getnframes()) != (1, 2, 24_000, samples):
            raise ValueError("actual PCM audio differs from the exported sample count")
        if len(audio.readframes(samples + 1)) != samples * 2:
            raise ValueError("truncated episode PCM data")
    return {"observed": observed, "video_sha256": file_hash(directory / "sequence.mp4"),
            "audio_sha256": file_hash(directory / "audio.wav"),
            "pipeline_sha256": file_hash(directory / "pipeline-report.json", MAX_JSON),
            "actual_audio_samples": samples, "pipeline": pipeline}


def join_audio(episodes: list[tuple[Path, int]], output: Path) -> dict:
    """Join exact PCM and pad only the final fraction of each video frame.

    At 24fps/24kHz each frame is exactly 1000 samples. AAC packet padding must
    not accumulate at compilation boundaries. No spoken sample is truncated.
    """
    if not 1 <= len(episodes) <= 5 or output.exists():
        raise ValueError("compilation needs 1..5 episodes and a new WAV path")
    total, padding = 0, []
    with wave.open(str(output), "wb") as target:
        target.setparams((1, 2, 24_000, 0, "NONE", "not compressed"))
        for path, frames in episodes:
            if not 720 <= frames <= 1440:
                raise ValueError("episode frame count outside 30..60seconds")
            with wave.open(str(path), "rb") as source:
                if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getcomptype()) != (1, 2, 24_000, "NONE"):
                    raise ValueError("compilation audio requires mono24kHz PCM16")
                actual = source.getnframes()
                pad = frames * 1000 - actual
                if not 0 <= pad < 1000:
                    raise ValueError("would truncate speech or pad more than a partial frame")
                remaining = actual
                while remaining:
                    count = min(remaining, 32_768)
                    data = source.readframes(count)
                    if len(data) != count * 2:
                        raise ValueError("truncated compilation source audio")
                    target.writeframesraw(data)
                    remaining -= count
                target.writeframesraw(b"\0\0" * pad)
                total += actual + pad
                padding.append(pad)
    return {"sample_rate": 24_000, "sample_count": total, "tail_padding_samples": padding,
            "spoken_audio_truncated": False, "sha256": file_hash(output)}


def verify_episode_hashes(directory: Path, receipt: dict):
    for filename, key in (("sequence.mp4", "video_sha256"), ("audio.wav", "audio_sha256"),
                          ("pipeline-report.json", "pipeline_sha256")):
        if file_hash(directory / filename) != receipt.get(key):
            raise ValueError(f"completed episode changed during compilation: {directory.name}/{filename}")


def verify_joined_audio(path: Path, receipt: dict, samples: int):
    if (receipt.get("sample_count") != samples or receipt.get("sample_rate") != 24_000
            or file_hash(path) != receipt.get("sha256")):
        raise ValueError("compilation PCM audio differs from its receipt")
    with wave.open(str(path), "rb") as audio:
        if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getnframes(),
                audio.getcomptype()) != (1, 2, 24_000, samples, "NONE"):
            raise ValueError("compilation PCM format or sample clock differs from its receipt")
        remaining = samples
        while remaining:
            count = min(remaining, 32_768)
            if len(audio.readframes(count)) != count * 2:
                raise ValueError("compilation PCM audio is truncated")
            remaining -= count


def unapproved_receipt(receipt: dict):
    if (receipt.get("schema_version") != 1 or receipt.get("publication_approved") is not False
            or receipt.get("acceptance") != UNMEASURED):
        raise ValueError("a render receipt cannot grant production or publication approval")


def render_compilation_video(ffmpeg: Path, ffprobe: Path, concat: Path, audio: Path,
                             video: Path, logs: Path, width: int, height: int, frames: int) -> dict:
    # MP4/AAC priming can offset concat input timestamps. Rebase the decoded
    # video on its exact frame count and encode all frames. `-shortest` with
    # stream copy drops delayed H.264 B-frames at the audio boundary.
    command = [str(ffmpeg), "-v", "error", "-nostdin", "-n", "-f", "concat", "-safe", "1",
               "-i", str(concat), "-i", str(audio), "-map", "0:v:0", "-map", "1:a:0",
               "-vf", "setpts=N/(24*TB)", "-r", "24", "-fps_mode", "cfr",
               "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-c:a", "aac",
               "-movflags", "+faststart", "-fs", str(MAX_MEDIA), str(video)]
    run_bounded(command, logs, "compilation-encode", 180)
    observed = inspect_media(probe(ffprobe, video, logs, "compilation-probe"), width, height, frames)
    return observed


def execute(args) -> dict:
    started = time.monotonic()
    if args.season.is_symlink() or not args.season.is_dir():
        raise ValueError("season must be an existing nonsymlink source directory")
    season = args.season.resolve()
    manifest = load_manifest(season)
    exporter, ffmpeg, ffprobe = (p.resolve(strict=True) for p in (args.exporter, args.ffmpeg, args.ffprobe))
    for tool in (exporter, ffmpeg, ffprobe):
        if not os.access(tool, os.X_OK):
            raise ValueError(f"tool is not executable: {tool}")
    profile = args.piper_config.absolute()
    width, height = map(int, args.review_size.split("x")) if args.review_size else (1920, 1080)
    identity = input_identity(season, exporter, profile, ffmpeg, ffprobe, args.group, width, height)
    output = args.output.absolute()
    marker = output / "render-inputs.json"
    if output.exists():
        if output.is_symlink() or not output.is_dir() or not args.resume or read_json(marker) != identity:
            raise ValueError("output exists: resume requires an owned directory with exactly matching inputs")
    else:
        if args.resume:
            raise ValueError("resume requires an existing owned render directory")
        output.mkdir(parents=True)
        write_json(marker, identity)
    logs = output / "logs"
    if logs.is_symlink():
        raise ValueError("log directory cannot be a symbolic link")
    logs.mkdir(exist_ok=True)
    group = manifest["youtube_groups"][args.group - 1]
    report = {"schema_version": 1, "status": "failed", "group": args.group,
              "review": args.review_size is not None, "publication_approved": False,
              "acceptance": UNMEASURED, "episodes": [], "inputs": identity}
    try:
        for number in group["episodes"]:
            entry = manifest["episodes"][number - 1]
            directory = output / f"ep{number:02}"
            receipt_path = directory / "render-receipt.json"
            resumed = directory.exists()
            if resumed:
                if not args.resume or directory.is_symlink():
                    raise ValueError(f"episode output already exists: {directory}")
                previous = read_json(receipt_path)
                unapproved_receipt(previous)
                if previous.get("inputs") != identity or previous.get("project_sha256") != entry["project_sha256"]:
                    raise ValueError("resume receipt does not match the exact render inputs")
                evidence = validate_episode(directory, width, height, ffprobe, logs, f"ep{number:02}-resume-probe")
                if any(previous.get(k) != evidence[k] for k in ("video_sha256", "audio_sha256", "pipeline_sha256")):
                    raise ValueError("completed media changed; resume is refused")
                receipt = previous
            else:
                print(f"Rendering {number:02}/31: {entry['title']}", flush=True)
                command = [str(exporter), "--project", str(season / entry["project"]),
                           "--piper-config", str(profile), "--ffmpeg", str(ffmpeg),
                           "--output", str(directory), "--retime-dialogue", "--review-frames"]
                if args.review_size:
                    command += ["--review-size", args.review_size]
                run_bounded(command, logs, f"ep{number:02}-export", 330)
                evidence = validate_episode(directory, width, height, ffprobe, logs, f"ep{number:02}-probe")
                if load_manifest(season) != manifest or input_identity(season, exporter, profile, ffmpeg, ffprobe, args.group, width, height) != identity:
                    raise ValueError("source, voice or selected executable changed during rendering")
                receipt = {"schema_version": 1, "number": number, "title": entry["title"],
                           "project_sha256": entry["project_sha256"], "inputs": identity,
                           "publication_approved": False, "acceptance": UNMEASURED, **evidence}
                write_json(receipt_path, receipt)
            report["episodes"].append({"number": number, "directory": directory.name,
                                       "completed_media_reused": resumed,
                                       "observed": receipt["observed"],
                                       "receipt_sha256": file_hash(receipt_path, MAX_JSON)})
        # Keep compilation candidates in their own directory. A failed attempt is
        # retained for diagnosis; resume never silently overwrites it.
        compilation = output / "compilation"
        completion = compilation / "render-receipt.json"
        receipts = [read_json(output / e["directory"] / "render-receipt.json") for e in report["episodes"]]
        for episode, receipt in zip(report["episodes"], receipts):
            verify_episode_hashes(output / episode["directory"], receipt)
        frames = sum(e["observed"]["frame_count"] for e in receipts)
        if compilation.exists():
            if not args.resume or compilation.is_symlink():
                raise ValueError("compilation output already exists")
            compiled = read_json(completion)
            unapproved_receipt(compiled)
            if compiled.get("episode_receipts") != report["episodes"] or compiled.get("inputs") != identity:
                # Reuse status is operational, not part of content identity.
                expected = [{k: v for k, v in e.items() if k != "completed_media_reused"} for e in report["episodes"]]
                actual = [{k: v for k, v in e.items() if k != "completed_media_reused"} for e in compiled.get("episode_receipts", [])]
                if actual != expected or compiled.get("inputs") != identity:
                    raise ValueError("compilation inputs changed")
            if file_hash(compilation / "sequence.mp4") != compiled.get("video_sha256"):
                raise ValueError("compilation media hash changed")
            observed = inspect_media(probe(ffprobe, compilation / "sequence.mp4", logs, "compilation-resume-probe"), width, height, frames)
            if observed != compiled.get("observed"):
                raise ValueError("actual compilation observations differ from its receipt")
        else:
            compilation.mkdir()
            joined = join_audio([(output / e["directory"] / "audio.wav", e["observed"]["frame_count"])
                                 for e in report["episodes"]], compilation / "audio.wav")
            concat = output / "compilation-inputs.txt"
            with concat.open("x") as target:
                for e in report["episodes"]:
                    target.write(f"file '{e['directory']}/sequence.mp4'\n")
                    target.write(f"duration {e['observed']['frame_count'] / 24:.12f}\n")
            observed = render_compilation_video(ffmpeg, ffprobe, concat, compilation / "audio.wav",
                                                compilation / "sequence.mp4", logs, width, height, frames)
            compiled = {"schema_version": 1, "group": args.group, "inputs": identity,
                        "episode_receipts": report["episodes"], "observed": observed,
                        "audio": joined, "video_sha256": file_hash(compilation / "sequence.mp4"),
                        "publication_approved": False, "acceptance": UNMEASURED}
            write_json(completion, compiled)
        verify_joined_audio(compilation / "audio.wav", compiled.get("audio", {}), frames * 1000)
        for episode, receipt in zip(report["episodes"], receipts):
            verify_episode_hashes(output / episode["directory"], receipt)
        if load_manifest(season) != manifest or input_identity(season, exporter, profile, ffmpeg, ffprobe, args.group, width, height) != identity:
            raise ValueError("source, voice or executable changed during compilation validation")
        report.update(status="completed", compilation=compiled)
    except (OSError, ValueError, KeyError, TypeError, ZeroDivisionError, wave.Error, subprocess.SubprocessError) as error:
        report["error"] = str(error)
    report["elapsed_seconds"] = time.monotonic() - started
    # Each attempt gets a new report; an earlier successful receipt remains intact.
    report_path = output / f"group-report-{time.time_ns()}.json"
    write_json(report_path, report)
    print(json.dumps({"status": report["status"], "group": args.group, "report": str(report_path),
                      "episodes_checked": len(report["episodes"]), "publication_approved": False,
                      "error": report.get("error")}), flush=True)
    return report


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--season", type=Path, default=Path(__file__).resolve().parents[1] / "examples/kilsat-season")
    parser.add_argument("--group", type=int, choices=range(1, 8), required=True)
    for key in ("exporter", "piper-config", "ffmpeg", "ffprobe", "output"):
        parser.add_argument("--" + key, type=Path, required=True)
    parser.add_argument("--review-size", choices=("960x540", "1280x720"))
    parser.add_argument("--resume", action="store_true", help="Revalidate completed receipts and reuse exact media; does not certify a speech runtime cache")
    args = parser.parse_args(argv)
    try:
        report = execute(args)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Season render failed: {error}\n")
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
