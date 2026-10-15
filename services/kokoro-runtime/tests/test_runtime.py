import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

RUNTIME = Path(__file__).resolve().parents[1] / 'runtime.py'
spec = importlib.util.spec_from_file_location('runtime', RUNTIME)
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.asset = self.root / 'local.asset'
        self.asset.write_text('mock')
        self.payload = dict(protocol=1, request=dict(text='Hello', voice=dict(preset='af_heart', language='en-us', speed=1.0)),
                            config_path=str(self.asset), model_path=str(self.asset), voice_path=str(self.asset), output_path=str(self.root/'out.wav'))

    def tearDown(self):
        self.temp.cleanup()

    def test_valid_without_loading_dependencies(self):
        self.assertEqual(runtime.validate(self.payload)['text'], 'Hello')

    def test_nonfinite_speed_and_missing_asset(self):
        self.payload['request']['voice']['speed'] = float('nan')
        with self.assertRaisesRegex(ValueError, 'speed'):
            runtime.validate(self.payload)
        self.payload['request']['voice']['speed'] = 1.0
        self.payload['model_path'] = str(self.root/'missing')
        with self.assertRaisesRegex(ValueError, 'missing local asset'):
            runtime.validate(self.payload)

    def test_malformed_request_returns_error_file(self):
        request, response = self.root/'request.json', self.root/'response.json'
        request.write_text(json.dumps({'protocol': 99}))
        run = subprocess.run([sys.executable, str(RUNTIME), '--request', str(request), '--response', str(response)], timeout=5, capture_output=True)
        self.assertEqual(run.returncode, 1)
        self.assertEqual(run.stdout, b'')
        self.assertIn('unsupported protocol', json.loads(response.read_text())['error'])

    def test_utf8_limit_and_path_traversal_preset(self):
        self.payload['request']['text'] = 'ä' * 8001
        with self.assertRaises(ValueError): runtime.validate(self.payload)
        self.payload['request']['text'] = 'Hi'
        self.payload['request']['voice']['preset'] = '../secret'
        with self.assertRaises(ValueError): runtime.validate(self.payload)


if __name__ == '__main__': unittest.main()
