import copy
import importlib.util
import json
from pathlib import Path
import shutil
import struct
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
                                     (1, "duration", "35.0"), (1, "channels", 2)):
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

    def test_subprocess_failure_and_timeout_do_not_report_a_completed_episode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "exited 7"):
                season.run_bounded([sys.executable, "-c", "raise SystemExit(7)"], root, "failed", 5)
            with self.assertRaises(TimeoutError):
                season.run_bounded([sys.executable, "-c", "import time; time.sleep(2)"], root, "timeout", 0.1)


if __name__ == "__main__":
    unittest.main()
