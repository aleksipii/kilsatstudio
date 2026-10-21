"""Voice selection preflight; no model, network, or audio inference required."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "night_room_real_speech", Path(__file__).parents[1] / "night_room_real_speech.py")
speech = importlib.util.module_from_spec(spec)
spec.loader.exec_module(speech)


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.assets = Path(self.temp.name)
        (self.assets / "voices").mkdir()
        for name in ("config.json", "kokoro-v1_0.pth", "voices/am_michael.pt",
                     "voices/af_heart.pt", "voices/bm_george.pt", "voices/am_adam.pt"):
            (self.assets / name).touch()
        self.voices = {"DAD": "am_michael", "STAN": "am_adam"}

    def test_lists_only_local_english_files(self):
        (self.assets / "voices/jf_alpha.pt").touch()
        (self.assets / "voices/am_directory.pt").mkdir()
        self.assertEqual(speech.installed_voices(self.assets),
                         ["af_heart", "am_adam", "am_michael", "bm_george"])

    def test_explicit_installed_male_boy_selection(self):
        speech.validate_selection(self.assets, self.voices, "en-us", 1.0)

    def test_missing_or_path_voice_is_rejected(self):
        for voice in ("am_missing", "../am_adam", "/tmp/am_adam"):
            with self.subTest(voice=voice), self.assertRaisesRegex(ValueError, "not installed"):
                speech.validate_selection(self.assets, {"STAN": voice}, "en-us", 1)

    def test_language_mismatch_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "does not match"):
            speech.validate_selection(self.assets, self.voices, "en-gb", 1)
        speech.validate_selection(self.assets, {"STAN": "bm_george"}, "en-gb", 1)

    def test_nonfinite_and_out_of_range_speed_rejected(self):
        for speed in (float("nan"), float("inf"), 0.49, 2.01):
            with self.subTest(speed=speed), self.assertRaisesRegex(ValueError, "speed"):
                speech.validate_selection(self.assets, self.voices, "en-us", speed)

    def test_model_missing_fails_before_inference(self):
        (self.assets / "kokoro-v1_0.pth").unlink()
        with self.assertRaisesRegex(ValueError, "missing local asset"):
            speech.validate_selection(self.assets, self.voices, "en-us", 1)


if __name__ == "__main__":
    unittest.main()
