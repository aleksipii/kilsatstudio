import importlib.util
from pathlib import Path
import unittest

PATH = Path(__file__).resolve().parents[1] / "scripts/benchmark_animation_pipeline.py"
spec = importlib.util.spec_from_file_location("benchmark_animation_pipeline", PATH)
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)


class BenchmarkTests(unittest.TestCase):
    def fixture(self):
        pipeline = {"schema_version": 1, "status": "completed", "review": False,
                    "streaming": {"width": 1920, "height": 1080, "fps": 24,
                                  "frame_count": 960, "total_samples": 960000}}
        media = {"streams": [{"codec_type": "video", "codec_name": "h264",
                              "width": 1920, "height": 1080, "avg_frame_rate": "24/1", "nb_read_frames": "960"},
                             {"codec_type": "audio", "codec_name": "aac", "sample_rate": "24000", "channels": 1}],
                 "format": {"duration": "40.0"}}
        hardware = {"os": "Darwin", "ci": False, "virtualized": 0}
        return pipeline, media, hardware

    def test_actual_profile_time_and_physical_environment_are_separate_gates(self):
        pipeline, media, hardware = self.fixture()
        result = benchmark.assess(pipeline, media, 299.0, hardware)
        self.assertTrue(result["media_valid"])
        self.assertTrue(result["numeric_300_second_budget_met"])
        self.assertFalse(result["physical_mac_timing_accepted"])
        result = benchmark.assess(pipeline, media, 299.0, hardware, True)
        self.assertTrue(result["physical_mac_timing_accepted"])
        self.assertFalse(result["distribution_approved"])
        self.assertEqual(result["speech_quality"], "unmeasured")

    def test_review_or_ci_or_virtual_mac_cannot_satisfy_physical_full_hd_target(self):
        pipeline, media, hardware = self.fixture()
        pipeline["review"] = True
        self.assertFalse(benchmark.assess(pipeline, media, 50, hardware, True)["numeric_300_second_budget_met"])
        pipeline["review"] = False
        for field in ["ci", "virtualized"]:
            changed = dict(hardware, **{field: 1})
            self.assertFalse(benchmark.assess(pipeline, media, 50, changed, True)["physical_mac_timing_accepted"])

    def test_actual_decoded_frames_dimensions_and_codecs_must_match(self):
        for key, value in [("nb_read_frames", "959"), ("width", 640), ("avg_frame_rate", "12/1"), ("codec_name", "vp9")]:
            pipeline, media, hardware = self.fixture()
            media["streams"][0][key] = value
            result = benchmark.assess(pipeline, media, 20, hardware, True)
            self.assertFalse(result["media_valid"], key)
            self.assertFalse(result["physical_mac_timing_accepted"], key)
        pipeline, media, hardware = self.fixture()
        self.assertFalse(benchmark.assess(pipeline, {}, 20, hardware, True)["media_valid"])

    def test_outer_elapsed_overrides_an_exporters_optimistic_timing(self):
        pipeline, media, hardware = self.fixture()
        pipeline["elapsed_seconds"] = 10
        pipeline["within_deadline"] = True
        result = benchmark.assess(pipeline, media, 300.5, hardware, True)
        self.assertTrue(result["media_valid"])
        self.assertFalse(result["numeric_300_second_budget_met"])
        self.assertFalse(result["physical_mac_timing_accepted"])


if __name__ == "__main__":
    unittest.main()
