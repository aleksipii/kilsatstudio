#!/usr/bin/env python3
"""Opt-in real CPU smoke test using preinstalled assets; never downloads assets."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import wave


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-local-inference', action='store_true', required=True,
                        help='explicitly opt in to loading local weights and synthesizing speech')
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--voice', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=float, default=180)
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        parser.error('this smoke runner requires macOS; Linux unit tests do not verify macOS support')
    if not 0 < args.timeout <= 3600:
        parser.error('timeout must be between 0 and 3600 seconds')
    output = args.output.resolve()
    if output.exists():
        parser.error('output already exists; choose a new WAV path')
    payload = dict(protocol=1, request=dict(text='Hello from Kilsat Studio. This is a local CPU speech test.',
                   voice=dict(preset=args.voice.stem, language='en-us', speed=1.0)),
                   config_path=str(args.config.resolve()), model_path=str(args.model.resolve()),
                   voice_path=str(args.voice.resolve()), output_path=str(output))
    with tempfile.TemporaryDirectory(prefix='kokoro-smoke-') as temporary:
        request, response = Path(temporary)/'request.json', Path(temporary)/'response.json'
        request.write_text(json.dumps(payload), encoding='utf-8')
        try:
            process = subprocess.run([sys.executable, str(Path(__file__).with_name('runtime.py')),
                                      '--request', str(request), '--response', str(response)], timeout=args.timeout)
        except subprocess.TimeoutExpired:
            print('Local CPU inference timed out; no compatibility claim established.', file=sys.stderr)
            return 1
        if not response.is_file():
            print('Runtime did not produce a response.', file=sys.stderr)
            return 1
        result = json.loads(response.read_text(encoding='utf-8'))
        if process.returncode or not result.get('ok'):
            print(result.get('error', 'Local inference failed'), file=sys.stderr)
            return 1
    with wave.open(str(output)) as wav:
        if (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) != (1, 2, 24000) or wav.getnframes() != result['sample_count'] or not wav.getnframes():
            raise ValueError('runtime returned an invalid PCM WAV')
        duration = wav.getnframes() / wav.getframerate()
    print(json.dumps(dict(platform=platform.platform(), architecture=platform.machine(),
                          backend='Python/PyTorch CPU', output=str(output), duration_seconds=duration,
                          alignment='not_aligned', perceptual_validation='requires listening')))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
