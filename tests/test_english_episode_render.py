import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("render_kilsat_english", ROOT / "scripts/render_kilsat_english.py")
with mock.patch.object(sys, "path", [str(ROOT / "scripts"), *sys.path]):
    english = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(english)


def write(path, value):
    path.write_text(json.dumps(value))


def fixture(root):
    season = root / "season"
    season.mkdir()
    (season / "assets").mkdir()
    asset = season / "assets/example.png"
    asset.write_bytes(b"test-only source asset")
    manifest = {"schema_version": 1, "language": "en-us", "profile": english.PROFILE,
                "publishable": False, "acceptance": english.UNMEASURED,
                "asset_sha256": {"assets/example.png": english.file_hash(asset)}, "episodes": []}
    for number in (1, 2, 3):
        source = {"project": {"captions": {"enabled": True}},
                  "characters": {actor: {"voice": {"provider": "kokoro", "language": "en-us", "preset": preset}}
                                 for actor, preset in english.VOICES.items()},
                  "scenes": [{"id": f"line-{index}", "events": [{"type": "speak", "actor": "AINO" if index % 2 == 0 else "RANE", "text": "This is a fixture line.", "audio": None}]}
                             for index in range(8)]}
        project = {"source": json.dumps(source), "assets": [{"path": "assets/example.png"}]}
        name = f"ep{number:02d}.kilsat"
        write(season / name, project)
        manifest["episodes"].append({"number": number, "project": name, "title": f"Fixture {number}",
                                     "project_sha256": english.file_hash(season / name),
                                     "source_sha256": english.hashlib.sha256(project["source"].encode()).hexdigest()})
    write(season / "manifest.json", manifest)
    for name in ("python-real", "runtime.py", "config.json", "model.pth", "exporter", "ffmpeg", "ffprobe"):
        (root / name).write_bytes(f"test-only {name}".encode())
        (root / name).chmod(0o700)
    (root / "venv-python").symlink_to(root / "python-real")
    (root / "voices").mkdir()
    for preset in english.VOICES.values():
        (root / "voices" / f"{preset}.pt").write_bytes(f"test-only {preset}".encode())
    profile = {"python": str(root / "venv-python"), "runtime_script": str(root / "runtime.py"),
               "model_config": str(root / "config.json"), "model_weights": str(root / "model.pth"),
               "voices_directory": str(root / "voices"), "cache_directory": str(root / "cache"),
               "model_version": "fixture-only", "timeout_seconds": 120}
    write(root / "profile.json", profile)
    return season, manifest, profile


class EnglishRenderTests(unittest.TestCase):
    def test_three_sources_are_hash_bound_and_remain_unapproved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, _ = fixture(root)
            manifest, dialogue = english.load_manifest(season)
            self.assertEqual([e["number"] for e in manifest["episodes"]], [1, 2, 3])
            self.assertFalse(manifest["publishable"])
            self.assertTrue(all(value is False for value in manifest["acceptance"].values()))
            self.assertEqual(len(dialogue["ep01.kilsat"]), 8)
            (season / "ep01.kilsat").write_text("changed")
            with self.assertRaisesRegex(ValueError, "project changed"):
                english.load_manifest(season)

    def test_foreign_asset_path_and_changed_assets_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, manifest, _ = fixture(root)
            path = season / "assets/example.png"
            data = path.read_bytes()
            path.write_bytes(data + b"changed")
            with self.assertRaisesRegex(ValueError, "asset changed"):
                english.load_manifest(season)
            path.write_bytes(data)
            manifest["asset_sha256"] = {"../escape.png": "0" * 64}
            write(season / "manifest.json", manifest)
            with self.assertRaisesRegex(ValueError, "inside"):
                english.load_manifest(season)

    def test_symlink_asset_and_output_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, _ = fixture(root)
            path = season / "assets/example.png"
            path.unlink()
            path.symlink_to(root / "model.pth")
            with self.assertRaisesRegex(ValueError, "symbolic"):
                english.load_manifest(season)
            (root / "linked").symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symbolic"):
                english.safe_path(root / "linked/output")

    def test_manifest_cannot_claim_quality_approval_or_change_membership(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, original, _ = fixture(root)
            for change in ("approval", "language", "membership"):
                manifest = copy.deepcopy(original)
                if change == "approval":
                    manifest["acceptance"]["artistic_review"] = True
                elif change == "language":
                    manifest["language"] = "fi-fi"
                else:
                    manifest["episodes"].pop()
                write(season / "manifest.json", manifest)
                with self.assertRaises(ValueError):
                    english.load_manifest(season)

    def test_source_voice_and_imported_audio_are_explicit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, _ = fixture(root)
            original = english.read_json(season / "ep01.kilsat")
            for change in ("voice", "imported", "caption", "actor"):
                project = copy.deepcopy(original)
                source = json.loads(project["source"])
                if change == "voice":
                    source["characters"]["RANE"]["voice"]["language"] = "fi-fi"
                elif change == "imported":
                    source["scenes"][0]["events"][0]["audio"] = "foreign.wav"
                elif change == "caption":
                    source["project"]["captions"]["enabled"] = False
                else:
                    source["scenes"][0]["events"][0]["actor"] = "RANE"
                project["source"] = json.dumps(source)
                with self.assertRaises(ValueError):
                    english.expected_dialogue(project)

    def test_identity_preserves_venv_python_and_detects_changed_voice(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, _ = fixture(root)
            def identity():
                return english.input_identity(season, root / "profile.json", root / "exporter", root / "ffmpeg", root / "ffprobe")
            first = identity()
            self.assertEqual(first["speech_files"]["python"]["path"], str(root / "venv-python"))
            self.assertEqual(first["speech_files"]["python"]["resolved_path"], str(root / "python-real"))
            self.assertFalse(first["dependency_closure_verified"])
            self.assertFalse(first["publication_approved"])
            (root / "voices/af_heart.pt").write_bytes(b"changed fixture voice")
            self.assertNotEqual(first["voice_files"], identity()["voice_files"])
            (root / "model.pth").write_bytes(b"changed fixture model")
            self.assertNotEqual(first["speech_files"], identity()["speech_files"])

    def test_profile_rejects_relative_files_unbounded_timeout_and_unknown_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, original = fixture(root)
            for key, value in (("model_weights", "relative.pth"), ("timeout_seconds", 601), ("download_url", "unrequested")):
                profile = dict(original, **{key: value})
                write(root / "profile.json", profile)
                with self.assertRaises(ValueError):
                    english.input_identity(season, root / "profile.json", root / "exporter", root / "ffmpeg", root / "ffprobe")

    def test_real_dialogue_provenance_rejects_missing_foreign_or_empty_pcm(self):
        expected = [{"scene": "first", "actor": "AINO", "language": "en-us", "preset": "af_heart"}]
        observed = dict(expected[0], imported=False, sample_rate=24000, sample_count=1234, pcm_sha256="0" * 64)
        english.verify_dialogue({"dialogue_audio": [observed]}, expected)
        for key, value in (("language", "fi-fi"), ("preset", "piper_harri"), ("imported", True), ("sample_count", 0), ("pcm_sha256", "missing")):
            with self.assertRaises(ValueError):
                english.verify_dialogue({"dialogue_audio": [dict(observed, **{key: value})]}, expected)
        with self.assertRaises(ValueError):
            english.verify_dialogue({"dialogue_audio": []}, expected)

    def test_reuse_is_rejected_before_any_process_and_failure_is_not_completed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            season, _, _ = fixture(root)
            output = root / "output"
            output.mkdir()
            args = (season, root / "exporter", root / "profile.json", root / "ffmpeg", root / "ffprobe", output)
            with mock.patch.object(english, "run_bounded") as process:
                with self.assertRaisesRegex(ValueError, "new output"):
                    english.render(*args)
                process.assert_not_called()
            output.rmdir()
            with mock.patch.object(english, "run_bounded", side_effect=ValueError("fixture exporter failed")):
                report = english.render(*args)
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["episodes"], [])
            self.assertIn("fixture exporter failed", report["error"])
            self.assertEqual(english.read_json(output / "english-render-report.json")["status"], "failed")
            self.assertFalse((output / "ep01/render-receipt.json").exists())


if __name__ == "__main__":
    unittest.main()
