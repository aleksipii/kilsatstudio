import copy
import importlib.util
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import wave

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("render_kilsat_season", ROOT / "scripts/render_kilsat_season.py")
season = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(season)


class SeasonTests(unittest.TestCase):
    def test_all_31_original_projects_and_seven_compilations_are_hash_bound(self):
        manifest = season.load_manifest(ROOT / "examples/kilsat-season")
        self.assertEqual(len(manifest["episodes"]), 31)
        self.assertEqual([len(g["episodes"]) for g in manifest["youtube_groups"]], [5] * 6 + [1])
        self.assertFalse(manifest["publishable"])
        self.assertTrue(all(value is False for value in manifest["acceptance"].values()))

    def test_changed_source_asset_or_compilation_membership_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "season"
            shutil.copytree(ROOT / "examples/kilsat-season", root)
            project = root / "ep01.kilsat"
            original = project.read_bytes()
            project.write_bytes(original + b" ")
            with self.assertRaisesRegex(ValueError, "project changed"):
                season.load_manifest(root)
            project.write_bytes(original)
            asset = root / "assets/kilsat-example.png"
            image = asset.read_bytes()
            asset.write_bytes(image + b"x")
            with self.assertRaisesRegex(ValueError, "asset changed"):
                season.load_manifest(root)
            asset.write_bytes(image)
            manifest = json.loads((root / "manifest.json").read_text())
            manifest["youtube_groups"][0]["episodes"].append(6)
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "membership"):
                season.load_manifest(root)

    def test_paths_cannot_escape_or_follow_asset_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative in ("../escape", "/escape", "assets/../../escape", "assets\\escape", "assets/./escape"):
                with self.assertRaises(ValueError, msg=relative):
                    season.season_path(root, relative)
            (root / "assets").symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symbolic"):
                season.season_path(root, "assets/image.png")

    def test_input_receipt_detects_changed_model_and_executable_without_certifying_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "manifest.json").write_text("{}")
            for name in ("model", "config", "piper", "exporter", "ffmpeg", "ffprobe"):
                (root / name).write_bytes(name.encode())
            profile = root / "profile.json"
            profile.write_text(json.dumps({"executable": str(root / "piper"),
                                          "model_path": str(root / "model"),
                                          "config_path": str(root / "config"), "preset": "piper_harri",
                                          "environment_identity": "a claim"}))
            def identity():
                return season.input_identity(root, root / "exporter", profile, root / "ffmpeg", root / "ffprobe", 1, 960, 540)
            first = identity()
            self.assertFalse(first["dependency_closure_verified"])
            self.assertFalse(first["publication_approved"])
            (root / "model").write_bytes(b"changed weights")
            second = identity()
            self.assertNotEqual(first["speech_files"], second["speech_files"])
            (root / "exporter").write_bytes(b"changed binary")
            self.assertNotEqual(second["tools"], identity()["tools"])

    def media(self):
        return {"streams": [
            {"codec_type": "video", "codec_name": "h264", "width": 960, "height": 540,
             "avg_frame_rate": "24/1", "nb_read_frames": "960", "duration": "40.0"},
            {"codec_type": "audio", "codec_name": "aac", "sample_rate": "24000", "channels": 1,
             "duration": "40.0"}], "format": {"duration": "40.0"}}

    def test_actual_media_requires_decoded_frames_track_duration_and_requested_profile(self):
        media = self.media()
        self.assertEqual(season.inspect_media(media, 960, 540, 960)["frame_count"], 960)
        for stream, field, value in ((0, "nb_read_frames", "959"), (0, "width", 1920),
                                     (0, "avg_frame_rate", "12/1"), (1, "codec_name", "mp3"),
                                     (1, "duration", "35.0"), (1, "channels", 2),
                                     (0, "start_time", "0.041992")):
            changed = copy.deepcopy(media)
            changed["streams"][stream][field] = value
            with self.assertRaises(ValueError, msg=field):
                season.inspect_media(changed, 960, 540, 960)
        for duration in ("nan", "inf", "39.0"):
            changed = copy.deepcopy(media)
            changed["format"]["duration"] = duration
            with self.assertRaises(ValueError):
                season.inspect_media(changed, 960, 540, 960)

    def make_wav(self, path, samples, marker):
        data = struct.pack("<h", marker) * samples
        with wave.open(str(path), "wb") as output:
            output.setparams((1, 2, 24000, 0, "NONE", "not compressed"))
            output.writeframes(data)
        return data

    def test_compilation_preserves_every_pcm_sample_and_only_pads_partial_tail_frames(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = self.make_wav(root / "one.wav", 720000 - 317, 1234)
            second = self.make_wav(root / "two.wav", 721000, -2345)
            result = season.join_audio([(root / "one.wav", 720), (root / "two.wav", 721)], root / "joined.wav")
            self.assertEqual(result["sample_count"], 1441000)
            self.assertEqual(result["tail_padding_samples"], [317, 0])
            self.assertFalse(result["spoken_audio_truncated"])
            with wave.open(str(root / "joined.wav"), "rb") as output:
                self.assertEqual(output.readframes(1441000), first + b"\0\0" * 317 + second)

    def test_compilation_refuses_speech_truncation_excess_silence_and_truncated_pcm(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, samples in enumerate((720001, 719000)):
                self.make_wav(root / "input.wav", samples, 100)
                with self.assertRaisesRegex(ValueError, "truncate speech"):
                    season.join_audio([(root / "input.wav", 720)], root / f"invalid{index}.wav")
            self.make_wav(root / "input.wav", 720000, 100)
            data = (root / "input.wav").read_bytes()
            (root / "input.wav").write_bytes(data[:-100])
            with self.assertRaisesRegex(ValueError, "truncated"):
                season.join_audio([(root / "input.wav", 720)], root / "truncated.wav")

    def test_reused_compilation_requires_retained_pcm_bytes_and_exact_sample_clock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "audio.wav"
            self.make_wav(path, 720000, 321)
            receipt = {"sample_count": 720000, "sample_rate": 24000, "sha256": season.file_hash(path)}
            season.verify_joined_audio(path, receipt, 720000)
            self.make_wav(path, 720000, -321)
            with self.assertRaisesRegex(ValueError, "differs"):
                season.verify_joined_audio(path, receipt, 720000)
            receipt["sha256"] = season.file_hash(path)
            with self.assertRaisesRegex(ValueError, "differs"):
                season.verify_joined_audio(path, receipt, 721000)
            path.unlink()
            with self.assertRaises(ValueError):
                season.verify_joined_audio(path, receipt, 720000)

    def test_episode_media_changed_after_validation_cannot_enter_a_compilation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt = {}
            for filename, key in (("sequence.mp4", "video_sha256"), ("audio.wav", "audio_sha256"),
                                  ("pipeline-report.json", "pipeline_sha256")):
                (root / filename).write_bytes(filename.encode())
                receipt[key] = season.file_hash(root / filename)
            season.verify_episode_hashes(root, receipt)
            (root / "audio.wav").write_bytes(b"different spoken content")
            with self.assertRaisesRegex(ValueError, "changed during compilation"):
                season.verify_episode_hashes(root, receipt)

    def test_saved_receipts_cannot_turn_rendering_into_quality_or_rights_approval(self):
        receipt = {"schema_version": 1, "publication_approved": False, "acceptance": season.UNMEASURED.copy()}
        season.unapproved_receipt(receipt)
        receipt["publication_approved"] = True
        with self.assertRaises(ValueError):
            season.unapproved_receipt(receipt)
        receipt["publication_approved"] = False
        receipt["acceptance"]["artistic_review"] = True
        with self.assertRaises(ValueError):
            season.unapproved_receipt(receipt)

    @unittest.skipUnless(shutil.which("ffmpeg") and shutil.which("ffprobe"), "actual FFmpeg and ffprobe required")
    def test_real_h264_aac_compilation_retains_delayed_frames_on_the_shared_clock(self):
        ffmpeg, ffprobe = Path(shutil.which("ffmpeg")), Path(shutil.which("ffprobe"))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            episodes = []
            concat = root / "inputs.txt"
            for index, color in enumerate(("red", "blue")):
                directory = root / f"ep{index:02}"
                directory.mkdir()
                self.make_wav(directory / "audio.wav", 720000 - 317, 1234 + index)
                subprocess.run([str(ffmpeg), "-v", "error", "-nostdin", "-n", "-f", "lavfi",
                                "-i", f"color=c={color}:s=64x36:r=24:d=30", "-i", str(directory / "audio.wav"),
                                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-bf", "3", "-c:a", "aac",
                                str(directory / "sequence.mp4")], check=True, capture_output=True, timeout=30)
                episodes.append((directory / "audio.wav", 720))
            concat.write_text("file 'ep00/sequence.mp4'\nduration 30\nfile 'ep01/sequence.mp4'\nduration 30\n")
            season.join_audio(episodes, root / "joined.wav")
            observed = season.render_compilation_video(ffmpeg, ffprobe, concat, root / "joined.wav",
                                                      root / "compilation.mp4", root, 64, 36, 1440)
            self.assertEqual(observed["frame_count"], 1440)
            media = json.loads(subprocess.check_output([str(ffprobe), "-v", "error", "-show_streams",
                                                       "-of", "json", str(root / "compilation.mp4")]))
            for stream in media["streams"]:
                self.assertLessEqual(abs(float(stream["start_time"])), 1 / 24000)

    def test_subprocess_failure_and_timeout_do_not_report_a_completed_episode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "exited 7"):
                season.run_bounded([sys.executable, "-c", "raise SystemExit(7)"], root, "failed", 5)
            with self.assertRaises(TimeoutError):
                season.run_bounded([sys.executable, "-c", "import time; time.sleep(2)"], root, "timeout", 0.1)


if __name__ == "__main__":
    unittest.main()
