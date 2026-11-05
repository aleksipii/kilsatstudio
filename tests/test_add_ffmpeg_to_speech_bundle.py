import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/add_ffmpeg_to_speech_bundle.py'
spec = importlib.util.spec_from_file_location('add_ffmpeg', SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AddFfmpegTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.speech, self.ffmpeg = root / 'speech', root / 'ffmpeg'
        self.speech.mkdir()
        (self.speech / 'speech-bundle.json').write_text(json.dumps({'schema_version': 1, 'bundle_id': 'b'}))
        (self.ffmpeg / 'bin').mkdir(parents=True)
        for tool in ('ffmpeg', 'ffprobe'):
            (self.ffmpeg / 'bin' / tool).write_bytes(b'tool')
        (self.ffmpeg / 'ffmpeg-bundle.json').write_text(json.dumps(dict(
            schema_version=1, ffmpeg='bin/ffmpeg', ffprobe='bin/ffprobe', ffmpeg_version='9.0.2',
            x264_commit='abc', license='GPL-2.0-or-later')))

    def test_copies_ffmpeg_and_records_relative_paths(self):
        module.add(self.speech, self.ffmpeg)
        descriptor = json.loads((self.speech / 'speech-bundle.json').read_text())
        self.assertEqual(descriptor['bundle_id'], 'b')
        self.assertEqual(descriptor['ffmpeg']['executable'], 'ffmpeg/bin/ffmpeg')
        self.assertEqual(descriptor['ffmpeg']['ffprobe'], 'ffmpeg/bin/ffprobe')
        self.assertEqual((self.speech / 'ffmpeg/bin/ffprobe').read_bytes(), b'tool')

    def test_refuses_twice_and_missing_tools(self):
        module.add(self.speech, self.ffmpeg)
        with self.assertRaises(ValueError):
            module.add(self.speech, self.ffmpeg)
        (self.ffmpeg / 'bin/ffprobe').unlink()
        other = Path(self.temporary.name) / 'other'
        other.mkdir()
        (other / 'speech-bundle.json').write_text('{}')
        with self.assertRaises(ValueError):
            module.add(other, self.ffmpeg)
        self.assertFalse((other / 'ffmpeg').exists())


if __name__ == '__main__':
    unittest.main()
