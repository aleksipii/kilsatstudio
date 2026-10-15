#!/usr/bin/env python3
"""Opt-in real CPU smoke test using preinstalled assets; never downloads assets."""
import argparse
from array import array
import hashlib
from importlib import metadata
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import wave


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def analyze_wav(path, expected_frames):
    """Measure PCM structure and signal, without claiming intelligibility or alignment."""
    if type(expected_frames) is not int or expected_frames <= 0:
        raise ValueError('runtime response requires a positive integer sample_count')
    with wave.open(str(path), 'rb') as wav:
        if (wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getcomptype()) != (1, 2, 24000, 'NONE'):
            raise ValueError('runtime output must be 24kHz mono PCM16 WAV')
        if wav.getnframes() != expected_frames:
            raise ValueError('WAV frame count differs from runtime sample_count')
        count, peak, squares = 0, 0, 0
        while True:
            block = wav.readframes(16384)
            if not block:
                break
            values = array('h')
            values.frombytes(block)
            if sys.byteorder != 'little':
                values.byteswap()
            count += len(values)
            peak = max(peak, max((abs(v) for v in values), default=0))
            squares += sum(v * v for v in values)
        if count != expected_frames:
            raise ValueError('WAV sample data is truncated')
        if peak == 0:
            raise ValueError('runtime output contains only silence; inspect the local model and voice')
    return dict(sample_rate=24000, channels=1, bits_per_sample=16, sample_count=count,
                duration_seconds=count / 24000, nonzero_signal=True,
                peak=peak / 32768, rms=math.sqrt(squares / count) / 32768)


def publish_report(path, report):
    # Write fully before atomically reserving the final name; never replace an existing file.
    descriptor, temporary = tempfile.mkstemp(prefix='.kokoro-evidence-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w', encoding='utf-8') as target:
            json.dump(report, target, indent=2, allow_nan=False)
            target.write('\n')
            target.flush()
            os.fsync(target.fileno())
        os.link(temporary, path)
    finally:
        os.unlink(temporary)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-local-inference', action='store_true', required=True,
                        help='explicitly opt in to loading local weights and synthesizing speech')
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--voice', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, help='optional new JSON evidence file')
    parser.add_argument('--timeout', type=float, default=180)
    args = parser.parse_args(argv)
    if platform.system() != 'Darwin':
        parser.error('this smoke runner requires macOS; Linux unit tests do not verify macOS support')
    if not 0 < args.timeout <= 3600:
        parser.error('timeout must be between 0 and 3600 seconds')
    # absolute() keeps final symlinks visible for exclusive-creation checks.
    output = args.output.absolute()
    report_path = args.report.absolute() if args.report else None
    for label, path in [('output', output), ('report', report_path)]:
        if path is None:
            continue
        if os.path.lexists(path):
            parser.error(f'{label} already exists; choose a new path')
        if not path.parent.is_dir():
            parser.error(f'{label} parent directory is missing: {path.parent}')
    if report_path == output:
        parser.error('output and report must use different paths')
    assets = dict(config=args.config.resolve(), model=args.model.resolve(), voice=args.voice.resolve())
    for label, path in assets.items():
        if not path.is_file():
            parser.error(f'missing local {label} asset: {path}; install assets explicitly before running')
    try:
        fingerprints = {label: dict(path=str(path), sha256=sha256(path)) for label, path in assets.items()}
        # Stage on the output filesystem, so final publication is exclusive and atomic.
        with tempfile.TemporaryDirectory(prefix='.kokoro-smoke-', dir=output.parent) as temporary:
            request, response, staged = [Path(temporary) / name for name in ('request.json', 'response.json', 'speech.wav')]
            payload = dict(protocol=1, request=dict(text='Hello from Kilsat Studio. This is a local CPU speech test.',
                           voice=dict(preset=args.voice.stem, language='en-us', speed=1.0)),
                           config_path=str(assets['config']), model_path=str(assets['model']),
                           voice_path=str(assets['voice']), output_path=str(staged))
            request.write_text(json.dumps(payload), encoding='utf-8')
            process = subprocess.run([sys.executable, str(Path(__file__).with_name('runtime.py')),
                                      '--request', str(request), '--response', str(response)], timeout=args.timeout)
            if not response.is_file():
                raise ValueError('runtime did not produce a response; check the Python environment and runtime logs')
            try:
                result = json.loads(response.read_text(encoding='utf-8'))
            except (ValueError, UnicodeError) as error:
                raise ValueError('runtime returned malformed response JSON') from error
            if not isinstance(result, dict) or type(result.get('ok')) is not bool:
                raise ValueError('runtime returned a malformed response object')
            if process.returncode or not result['ok']:
                raise ValueError(str(result.get('error', 'local inference failed; inspect runtime logs')))
            signal = analyze_wav(staged, result.get('sample_count'))
            versions = {}
            for package in ('kokoro', 'torch', 'numpy', 'misaki', 'huggingface-hub'):
                try:
                    versions[package] = metadata.version(package)
                except metadata.PackageNotFoundError:
                    versions[package] = None
            evidence = dict(schema_version=1, platform=platform.platform(), os=platform.mac_ver()[0],
                            architecture=platform.machine(), chip=platform.processor() or platform.machine(), python=dict(version=platform.python_version(), executable=sys.executable),
                            packages=versions, backend='Python/PyTorch CPU', assets=fingerprints,
                            output=str(output), output_sha256=sha256(staged), audio=signal,
                            duration_seconds=signal['duration_seconds'], synthesis='structural_checks_passed',
                            alignment='not_aligned', perceptual_validation='requires listening',
                            limitations=['Listening quality unverified', 'Phoneme alignment and lip-sync quality unverified',
                                         'Device playback and animation synchronization unverified'])
            os.link(staged, output)
            if report_path:
                publish_report(report_path, evidence)
        print(json.dumps(evidence))
        return 0
    except subprocess.TimeoutExpired:
        print('Local CPU inference timed out; increase --timeout or inspect the local environment. No compatibility claim established.', file=sys.stderr)
    except (OSError, ValueError, EOFError, wave.Error) as error:
        print(f'Local CPU smoke failed: {error}. No compatibility claim established.', file=sys.stderr)
    return 1


if __name__ == '__main__':
    raise SystemExit(main())
