"""Real-speech workflow helpers; no network, model or inference required."""
import contextlib
import hashlib
import io
import importlib.util
import json
from pathlib import Path
import runpy
import struct
import tempfile
import unittest
import wave

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("real_speech_ci", ROOT / "scripts/real_speech_ci.py")
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)
setup = runpy.run_path(str(ROOT / "services/kokoro-runtime/setup_kokoro.py"))
BASE = json.loads((ROOT / "services/kokoro-runtime/setup-manifest.json").read_text())
CATALOG = json.loads((ROOT / "services/kokoro-runtime/english-voices.json").read_text())
WORKFLOW = ROOT / ".github/workflows/real-speech-reference.yml"


def write_wav(path, frames, rate=24000, channels=1):
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as stream:
        stream.setnchannels(channels)
        stream.setsampwidth(2)
        stream.setframerate(rate)
        stream.writeframes(struct.pack(f"<{frames * channels}h", *([100] * frames * channels)))


class CatalogTests(unittest.TestCase):
    def test_catalog_matches_setup_manifest_pins(self):
        self.assertEqual(CATALOG["model_revision"], BASE["model_revision"])
        pinned = {e["name"]: e for e in CATALOG["voices"]}
        for item in BASE["files"]:
            if item["path"].startswith("voices/"):
                entry = pinned[Path(item["path"]).stem]
                self.assertEqual((entry["sha256"], entry["size_bytes"]),
                                 (item["sha256"], item["size_bytes"]))

    def test_catalog_entries_are_well_formed(self):
        self.assertEqual(CATALOG["license"]["model_card_license_tag"], "apache-2.0")
        for entry in CATALOG["voices"]:
            self.assertRegex(entry["name"], ci.VOICE_NAME)
            self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(entry["url"].endswith(
                f"/resolve/{BASE['model_revision']}/voices/{entry['name']}.pt"))


class ManifestTests(unittest.TestCase):
    def test_defaults_keep_base_and_pass_installer_validation(self):
        derived = ci.derive_manifest(BASE, CATALOG, ["am_michael", "af_heart"], "en-us")
        self.assertEqual(sorted(i["path"] for i in derived["files"]),
                         sorted(i["path"] for i in BASE["files"]))
        setup["validate_manifest"](derived)

    def test_extra_voice_is_pinned_and_changes_cache_key(self):
        base_key = ci.canonical_key(ci.derive_manifest(BASE, CATALOG, ["am_michael"], "en-us"))
        derived = ci.derive_manifest(BASE, CATALOG, ["am_michael", "am_puck"], "en-us")
        setup["validate_manifest"](derived)
        item = next(i for i in derived["files"] if i["path"] == "voices/am_puck.pt")
        self.assertEqual(item["sha256"],
                         "dd1d8973f4ce4b7d8ae407c77a435f485dabc052081b80ea75c4f30b84f36223")
        self.assertNotEqual(base_key, ci.canonical_key(derived))
        self.assertEqual(ci.canonical_key(derived), ci.canonical_key(
            ci.derive_manifest(BASE, CATALOG, ["am_puck", "am_michael"], "en-us")))

    def test_rejects_unknown_unsafe_and_mismatched_voices(self):
        for voice, message in (("am_missing", "not pinned"), ("../am_puck", "invalid"),
                               ("jf_alpha", "invalid"), ("bm_george", "does not match")):
            with self.subTest(voice=voice), self.assertRaisesRegex(ValueError, message):
                ci.derive_manifest(BASE, CATALOG, [voice], "en-us")

    def test_rejects_revision_and_hash_disagreement(self):
        other = dict(CATALOG, model_revision="0" * 40)
        with self.assertRaisesRegex(ValueError, "different model revisions"):
            ci.derive_manifest(BASE, other, ["am_michael"], "en-us")
        tampered = json.loads(json.dumps(CATALOG))
        next(e for e in tampered["voices"] if e["name"] == "am_michael")["sha256"] = "f" * 64
        with self.assertRaisesRegex(ValueError, "hash differs"):
            ci.derive_manifest(BASE, tampered, ["am_michael"], "en-us")


class VerifyAndProvenanceTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.assets = self.root / "assets"
        (self.assets / "voices").mkdir(parents=True)
        files = []
        for name, data in (("config.json", b"{}"), ("voices/am_michael.pt", b"voice")):
            (self.assets / name).write_bytes(data)
            files.append({"path": name, "size_bytes": len(data),
                          "sha256": hashlib.sha256(data).hexdigest()})
        self.manifest = dict(BASE, files=files)

    def test_verify_accepts_matching_and_reports_every_problem(self):
        self.assertEqual(len(ci.verify_assets(self.manifest, self.assets)), 2)
        (self.assets / "config.json").write_bytes(b"{ }")
        (self.assets / "voices/am_michael.pt").unlink()
        with self.assertRaises(ValueError) as caught:
            ci.verify_assets(self.manifest, self.assets)
        self.assertIn("config.json: expected", str(caught.exception))
        self.assertIn("am_michael.pt: missing", str(caught.exception))

    def provenance(self, report):
        return ci.build_provenance(
            manifest=self.manifest, catalog=CATALOG, speech_report=report,
            wav_dir=self.root / "out/wav", artifact_dir=self.root / "out", assets=self.assets,
            voices={"DAD": "am_michael", "STAN": "af_heart"}, speed=1.0, language="en-us",
            tool_versions={"rustc": "rustc 1.x"}, timings={"synthesis": 6.0}, cache_hit=False,
            freeze_text="kokoro==0.9.4\n", environ={"GITHUB_ACTOR": "owner", "RUNNER_ARCH": "ARM64"})

    def test_provenance_records_hashes_rtf_and_licence(self):
        write_wav(self.root / "out/wav/audio/a.wav", 24000)
        write_wav(self.root / "out/wav/audio/b.wav", 48000)
        (self.root / "out/sequence.mp4").write_bytes(b"mp4")
        report = [{"audio": "audio/a.wav", "actor": "DAD", "voice": "am_michael", "text": "Hi.",
                   "wall_seconds": 2.0, "rtf": 2.0, "mouth_timing": "estimated"},
                  {"audio": "audio/b.wav", "actor": "STAN", "voice": "af_heart", "text": "Yo.",
                   "wall_seconds": 4.0, "rtf": 2.0}]
        doc = self.provenance(report)
        self.assertEqual(doc["synthesis"]["total_audio_seconds"], 3.0)
        self.assertEqual(doc["synthesis"]["real_time_factor"], 2.0)
        self.assertEqual(doc["model"]["license"]["model_card_license_tag"], "apache-2.0")
        self.assertEqual(doc["voices"]["upstream_traits"],
                         {"af_heart": "female", "am_michael": "male"})
        self.assertEqual(doc["consent"]["actor"], "owner")
        self.assertIn("sequence.mp4", [o["path"] for o in doc["outputs"]])
        self.assertEqual(doc["packages"]["installed_freeze"], ["kokoro==0.9.4"])

    def test_provenance_rejects_missing_empty_or_wrong_format_audio(self):
        with self.assertRaisesRegex(ValueError, "empty"):
            self.provenance([])
        entry = {"audio": "audio/a.wav", "actor": "DAD", "voice": "am_michael", "text": "Hi.",
                 "wall_seconds": 1.0, "rtf": 1.0}
        with self.assertRaises(FileNotFoundError):
            self.provenance([entry])
        write_wav(self.root / "out/wav/audio/a.wav", 100, rate=22050)
        with self.assertRaisesRegex(ValueError, "24 kHz mono PCM16"):
            self.provenance([entry])

    def test_cli_manifest_prints_key(self):
        out = self.root / "derived.json"
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            self.run_cli(out)
        self.assertRegex(stdout.getvalue(), r"^key=[0-9a-f]{64}$")
        self.assertIn("invalid voice name", stderr.getvalue())

    def run_cli(self, out):
        code = ci.main(["manifest", "--base", str(ROOT / "services/kokoro-runtime/setup-manifest.json"),
                        "--catalog", str(ROOT / "services/kokoro-runtime/english-voices.json"),
                        "--voice", "am_michael", "--voice", "am_fenrir", "--out", str(out)])
        self.assertEqual(code, 0)
        self.assertIn("voices/am_fenrir.pt", out.read_text())
        self.assertEqual(ci.main(["manifest", "--base", str(out), "--catalog",
                                  str(ROOT / "services/kokoro-runtime/english-voices.json"),
                                  "--voice", "zz_bad", "--out", str(out)]), 1)


class WorkflowTests(unittest.TestCase):
    def test_workflow_is_manual_only_and_consent_gated(self):
        text = WORKFLOW.read_text()
        on_block = text.split("\non:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertIn("workflow_dispatch:", on_block)
        for trigger in ("push", "pull_request", "schedule", "workflow_run"):
            self.assertNotRegex(on_block, rf"(?m)^  {trigger}:")
        self.assertIn("inputs.consent_download != true", text)
        self.assertIn("--allow-downloads", text)
        self.assertIn("path: ${{ runner.temp }}/kokoro", text)

    def test_voice_choices_match_catalog_and_defaults(self):
        text = WORKFLOW.read_text()
        names = [e["name"] for e in CATALOG["voices"]]
        options = "".join(f"          - {name}\n" for name in names)
        self.assertEqual(text.count(options), 2)
        self.assertIn("default: am_michael", text)
        self.assertIn("default: af_heart", text)


if __name__ == "__main__":
    unittest.main()
