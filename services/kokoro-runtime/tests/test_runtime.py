import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from types import SimpleNamespace, ModuleType
import contextlib
import struct
import wave

try:
    import numpy as np
except ImportError:
    np = None

RUNTIME = Path(__file__).resolve().parents[1] / 'runtime.py'
spec = importlib.util.spec_from_file_location('runtime', RUNTIME)
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.asset = self.root / 'local.asset'
        self.asset.write_text('mock')
        self.payload = dict(protocol=1, request=dict(text='Hello', voice=dict(preset='af_heart', language='en-us', speed=1.0)),
                            config_path=str(self.asset), model_path=str(self.asset), voice_path=str(self.asset), output_path=str(self.root/'out.wav'))

    def tearDown(self):
        self.temp.cleanup()


class ProtocolTests(RuntimeFixture):
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


    def test_output_cannot_replace_model(self):
        self.payload['output_path'] = self.payload['model_path']
        with self.assertRaisesRegex(ValueError, 'overwrite'):
            runtime.validate(self.payload)

    def test_payload_shapes_and_boolean_protocol(self):
        for payload in ([], dict(protocol=True), dict(protocol=1, request=[])):
            with self.assertRaises(ValueError):
                runtime.validate(payload)


@unittest.skipIf(np is None, 'NumPy is required for synthesis-path tests')
class SynthesisTests(RuntimeFixture):
    def upstream(self, chunks=None, package=True):
        class Tensor:
            dtype = 'float32'
            ndim = 3
            shape = (510, 1, 256)
            def __init__(self, data=None): self.data = data
            def detach(self): return self
            def cpu(self): return self
            def numpy(self): return np.array(self.data)
        torch = SimpleNamespace(Tensor=Tensor, float32='float32',
                                load=mock.Mock(return_value=Tensor()),
                                isfinite=lambda _: np.array([True]),
                                inference_mode=contextlib.nullcontext)
        spacy = ModuleType('spacy')
        spacy.cli = ModuleType('spacy.cli')
        spacy.cli.download = mock.Mock()
        spacy.util = SimpleNamespace(is_package=lambda _: package)
        model = mock.Mock()
        model.to.return_value = model
        model.eval.return_value = model
        chunks = [[0.0, 0.5], [-0.5, 2.0]] if chunks is None else chunks
        pipeline = mock.Mock(return_value=[SimpleNamespace(audio=Tensor(chunk), phonemes='hi') for chunk in chunks])
        kokoro = SimpleNamespace(KModel=mock.Mock(return_value=model), KPipeline=mock.Mock(return_value=pipeline))
        return dict(torch=torch, spacy=spacy, **{'spacy.cli': spacy.cli}, kokoro=kokoro), kokoro, torch

    def test_cpu_synthesis_writes_complete_pcm_and_unaligned_phonemes(self):
        modules, kokoro, torch = self.upstream()
        self.payload['request']['voice']['style'] = 'happy'
        with mock.patch.dict(sys.modules, modules), mock.patch.object(runtime, 'disable_network'):
            result = runtime.synthesize(self.payload)
        with wave.open(self.payload['output_path']) as wav:
            self.assertEqual((wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes()), (1, 2, 24000, 4))
            self.assertEqual(struct.unpack('<4h', wav.readframes(4)), (0, 16383, -16383, 32767))
        self.assertEqual(result['sample_count'], 4)
        self.assertEqual(result['alignment'], 'not_aligned')
        self.assertEqual(result['phonemes'], 'hi hi')
        self.assertTrue(result['warnings'])
        self.assertEqual(kokoro.KPipeline.call_args.kwargs['device'], 'cpu')
        self.assertEqual(torch.load.call_args.kwargs, dict(map_location='cpu', weights_only=True))

    def test_malformed_voice_tensor_fails_before_pipeline_execution(self):
        modules, kokoro, torch = self.upstream()
        torch.load.return_value.shape = (510, 256)
        torch.load.return_value.ndim = 2
        with mock.patch.dict(sys.modules, modules), mock.patch.object(runtime, 'disable_network'):
            with self.assertRaisesRegex(ValueError, 'embeddings'):
                runtime.synthesize(self.payload)
        kokoro.KPipeline.return_value.assert_not_called()

    def test_missing_g2p_fails_before_loading_model(self):
        modules, kokoro, _ = self.upstream(package=False)
        with mock.patch.dict(sys.modules, modules), mock.patch.object(runtime, 'disable_network'):
            with self.assertRaisesRegex(RuntimeError, 'missing local en_core_web_sm'):
                runtime.synthesize(self.payload)
        kokoro.KModel.assert_not_called()

    def test_empty_nonfinite_and_multichannel_output_preserves_existing_file(self):
        output = Path(self.payload['output_path'])
        output.write_bytes(b'existing audio')
        for chunks in ([], [[]], [[float('nan')]], [[[0.1, 0.2]]]):
            modules, _, _ = self.upstream(chunks)
            with mock.patch.dict(sys.modules, modules), mock.patch.object(runtime, 'disable_network'):
                with self.assertRaisesRegex(ValueError, 'audio'):
                    runtime.synthesize(self.payload)
            self.assertEqual(output.read_bytes(), b'existing audio')

    def test_network_guards_deny_requests(self):
        with mock.patch.object(runtime.socket.socket, 'connect'), mock.patch.object(runtime.socket.socket, 'connect_ex'), mock.patch.object(runtime.socket, 'create_connection'), mock.patch.object(runtime.socket, 'getaddrinfo'), mock.patch.dict(runtime.os.environ):
            runtime.disable_network()
            for operation in (runtime.socket.create_connection, runtime.socket.getaddrinfo):
                with self.assertRaisesRegex(RuntimeError, 'network disabled'):
                    operation('example.com', 443)
            self.assertEqual(runtime.os.environ['HF_HUB_OFFLINE'], '1')


if __name__ == '__main__': unittest.main()

