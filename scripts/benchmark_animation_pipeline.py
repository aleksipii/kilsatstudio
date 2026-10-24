#!/usr/bin/env python3
"""Measure actual project->speech->MP4 execution plus media validation.

No device, voice quality, legal or artistic acceptance is inferred. Run only
with explicitly installed local inference assets and an executable you selected.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import tempfile
import time

MAX_JSON = 1024 * 1024


def file_hash(path: Path, limit: int = 512 * 1024 * 1024) -> str:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError(f"not a bounded regular file: {path}")
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            total += len(chunk)
            if total > limit:
                raise ValueError("file grew beyond its hash budget")
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict:
    if path.is_symlink() or path.stat().st_size > MAX_JSON:
        raise ValueError("unsafe or oversized report")
    with path.open("rb") as source:
        content = source.read(MAX_JSON + 1)
    if len(content) > MAX_JSON:
        raise ValueError("oversized report")
    value = json.loads(content)
    if not isinstance(value, dict):
        raise ValueError("report must be a JSON object")
    return value


def machine_info() -> dict:
    ci = bool(os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"))
    result = {"os": platform.system(), "release": platform.release(),
              "machine": platform.machine(), "processor": platform.processor(),
              "logical_cpus": os.cpu_count(), "ci": ci,
              "virtualized": None, "mac_model": None, "memory_bytes": None}
    if sys.platform == "darwin":
        for key, field in [("hw.model", "mac_model"), ("hw.memsize", "memory_bytes"),
                           ("kern.hv_vmm_present", "virtualized")]:
            try:
                value = subprocess.check_output(["/usr/sbin/sysctl", "-n", key],
                                                timeout=5, text=True).strip()
                result[field] = int(value) if field != "mac_model" else value
            except (OSError, ValueError, subprocess.SubprocessError):
                pass
    return result


def fraction(value: str) -> float:
    numerator, separator, denominator = str(value).partition("/")
    return float(numerator) / float(denominator) if separator else float(numerator)


def assess(pipeline: dict, media: dict, elapsed: float, hardware: dict,
           physical_mac_attested: bool = False) -> dict:
    errors = []
    streaming = pipeline.get("streaming", {})
    video = next((s for s in media.get("streams", []) if s.get("codec_type") == "video"), {})
    audio = next((s for s in media.get("streams", []) if s.get("codec_type") == "audio"), {})
    if pipeline.get("schema_version") != 1 or pipeline.get("status") != "completed":
        errors.append("exporter did not report a completed pipeline")
    try:
        for key in ("width", "height"):
            if int(video[key]) != int(streaming[key]):
                errors.append(f"actual video {key} differs from the requested profile")
        fps = fraction(video.get("avg_frame_rate", video.get("r_frame_rate", "0")))
        if abs(fps - int(streaming["fps"])) > 0.0001:
            errors.append("actual video frame rate differs from the requested profile")
        frames = int(video.get("nb_read_frames", video.get("nb_frames", -1)))
        if frames != int(streaming["frame_count"]):
            errors.append("decoded video frame count differs from the exporter")
        duration = float(media.get("format", {}).get("duration", video.get("duration", "0")))
        expected = int(streaming["total_samples"]) / 24_000
        if abs(duration - expected) > 1 / max(fps, 1) + 0.08:
            errors.append("actual media duration differs from the shared sample clock")
        if video.get("codec_name") != "h264" or audio.get("codec_name") != "aac":
            errors.append("the output is not the requested H.264/AAC video")
        if int(audio.get("channels", 0)) != 1 or int(audio.get("sample_rate", 0)) != 24_000:
            errors.append("encoded audio is not mono24kHz")
    except (KeyError, TypeError, ValueError, ZeroDivisionError, OverflowError) as error:
        duration, fps = 0.0, 0.0
        errors.append(f"missing or invalid actual media observations: {error}")
    full_hd = (streaming.get("width"), streaming.get("height"), streaming.get("fps")) == (1920, 1080, 24)
    review = bool(pipeline.get("review") or streaming.get("review"))
    timing = not errors and full_hd and not review and 30 <= duration <= 60.05 and elapsed <= 300
    physical = (physical_mac_attested and hardware.get("os") == "Darwin"
                and not hardware.get("ci") and hardware.get("virtualized") == 0)
    return {"media_valid": not errors, "errors": errors, "review": review,
            "actual_duration_seconds": duration, "actual_fps": fps,
            "full_hd_24_30_to_60_seconds": full_hd and not review and 30 <= duration <= 60.05,
            "numeric_300_second_budget_met": timing,
            "physical_mac_timing_accepted": timing and physical,
            "physical_mac_attested": physical_mac_attested,
            "physical_mac_environment_observed": physical,
            "speech_quality": "unmeasured", "artistic_quality": "unmeasured",
            "distribution_approved": False}


def run_bounded(command: list[str], timeout: float, log_dir: Path) -> tuple[int, str, str]:
    if timeout <= 0:
        raise TimeoutError("whole-pipeline deadline has elapsed")
    stdout_path, stderr_path = log_dir / "stdout.log", log_dir / "stderr.log"
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                   start_new_session=(os.name == "posix"))
        started = time.monotonic()
        try:
            while process.poll() is None:
                if time.monotonic() - started > timeout:
                    raise TimeoutError("whole-pipeline subprocess deadline exceeded")
                if max(stdout_path.stat().st_size, stderr_path.stat().st_size) > MAX_JSON:
                    raise ValueError("subprocess log byte budget exceeded")
                time.sleep(0.05)
        except BaseException:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                process.kill()
            process.wait()
            raise
    return process.returncode, stdout_path.read_text(errors="replace"), stderr_path.read_text(errors="replace")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exporter", required=True, type=Path)
    parser.add_argument("--project", required=True, type=Path)
    parser.add_argument("--ffmpeg", required=True, type=Path)
    parser.add_argument("--ffprobe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--piper-config", type=Path)
    parser.add_argument("--kokoro-config", type=Path)
    parser.add_argument("--review-size")
    parser.add_argument("--retime-dialogue", action="store_true")
    parser.add_argument("--private-system-preview", action="store_true")
    parser.add_argument("--physical-mac", action="store_true", help="Operator attests a physical Mac; observed CI/VM cannot pass")
    parser.add_argument("--run-label", choices=["cold", "warm"], default="cold", help="Declared run label, not inferred cache state")
    args = parser.parse_args(argv)
    if args.report.exists() or args.output.exists():
        parser.error("report and render output must be new paths")
    for path in [args.exporter, args.ffmpeg, args.ffprobe]:
        if not path.is_absolute() or not path.is_file():
            parser.error("executables must be explicit absolute local paths")
    # Homebrew exposes selected tools through symlinks. Bind and execute their
    # actual files, rather than rejecting every ordinary macOS installation.
    args.exporter = args.exporter.resolve(strict=True)
    args.ffmpeg = args.ffmpeg.resolve(strict=True)
    args.ffprobe = args.ffprobe.resolve(strict=True)
    started = time.monotonic()
    report = {"schema_version": 1, "status": "failed", "run_label": args.run_label,
              "cache_state": "operator-declared label; actual provider behaviour is in the pipeline report",
              "distribution_approved": False}
    try:
        hardware = machine_info()
        before = file_hash(args.project, 8 * MAX_JSON)
        tools = {name: {"path": str(path), "sha256": file_hash(path)}
                 for name, path in [("exporter", args.exporter), ("ffmpeg", args.ffmpeg), ("ffprobe", args.ffprobe)]}
        command = [str(args.exporter), "--project", str(args.project.resolve()), "--ffmpeg", str(args.ffmpeg),
                   "--output", str(args.output.absolute()), "--deadline-seconds", "300"]
        for option, value in [("--piper-config", args.piper_config), ("--kokoro-config", args.kokoro_config), ("--review-size", args.review_size)]:
            if value is not None:
                command.extend([option, str(value)])
        if args.retime_dialogue:
            command.append("--retime-dialogue")
        if args.private_system_preview:
            command.append("--private-system-preview")
        with tempfile.TemporaryDirectory(prefix="kilsat-benchmark-") as temporary:
            log_dir = Path(temporary)
            code, _, stderr = run_bounded(command, 300 - (time.monotonic() - started), log_dir)
            if code:
                raise ValueError(f"exporter exited{code}: {stderr[-4096:]}")
            pipeline = read_json(args.output / "pipeline-report.json")
            code, stdout, stderr = run_bounded([str(args.ffprobe), "-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", str(args.output / "sequence.mp4")],
                                               300 - (time.monotonic() - started), log_dir)
            if code:
                raise ValueError(f"ffprobe validation failed: {stderr[-4096:]}")
            media = json.loads(stdout)
        after = file_hash(args.project, 8 * MAX_JSON)
        if before != after:
            raise ValueError("project source changed during the benchmark")
        media_sha = file_hash(args.output / "sequence.mp4", 1024 * MAX_JSON)
        elapsed = time.monotonic() - started
        verdict = assess(pipeline, media, elapsed, hardware, args.physical_mac)
        report.update(status="completed" if verdict["media_valid"] else "failed",
                      project_sha256=before, media_sha256=media_sha, tools=tools,
                      elapsed_seconds=elapsed, pipeline=pipeline, actual_media=media,
                      hardware=hardware, verdict=verdict)
    except (OSError, ValueError, TimeoutError, subprocess.SubprocessError) as error:
        report.update(error=str(error), elapsed_seconds=time.monotonic() - started)
    encoded = json.dumps(report, ensure_ascii=False, indent=2).encode()
    if len(encoded) > MAX_JSON:
        raise ValueError("benchmark report exceeds1MiB")
    with args.report.open("xb") as output:
        output.write(encoded)
    print(json.dumps({"status": report["status"], "report": str(args.report), "elapsed_seconds": report["elapsed_seconds"], "error": report.get("error")}))
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
