#!/usr/bin/env python3
"""One-shot, file-based JSON protocol v1; local CPU inference only.

The host owns timeout, cancellation, caching and consent for asset installation.
No package or model installer is invoked by this service.
"""
import argparse
import contextlib
import json
import math
import os
from pathlib import Path
import socket
import sys
import wave

MAX_TEXT = 16000


def validate(payload):
    if payload.get('protocol') != 1:
        raise ValueError('unsupported protocol version')
    request = payload['request']
    text = request['text']
    if not isinstance(text, str) or not text.strip() or len(text.encode('utf-8')) > MAX_TEXT:
        raise ValueError('text must contain 1..16000 UTF-8 bytes')
    voice = request['voice']
    speed = voice['speed']
    if isinstance(speed, bool) or not isinstance(speed, (float, int)) or not math.isfinite(speed) or not 0.5 <= speed <= 2:
        raise ValueError('speed must be finite and between 0.5 and 2')
    if voice['language'] not in ('en-us', 'en-gb'):
        raise ValueError('unsupported language; use en-us or en-gb')
    preset = voice['preset']
    if not isinstance(preset, str) or not preset or not all(c.isascii() and (c.isalnum() or c == '_') for c in preset):
        raise ValueError('invalid voice preset')
    for key in ('config_path', 'model_path', 'voice_path'):
        if not isinstance(payload[key], str) or not Path(payload[key]).is_file():
            raise ValueError(f'missing local asset: {key}; install approved assets before synthesis')
    if not isinstance(payload['output_path'], str) or not Path(payload['output_path']).parent.is_dir():
        raise ValueError('output directory does not exist')
    return request


def disable_network():
    # HF offline flags alone do not prevent a G2P dependency attempting a download.
    # Enforce no network for this process, including optional dependency fallbacks.
    def denied(*args, **kwargs):
        raise RuntimeError('network disabled: install Kokoro and English G2P assets explicitly before running')
    socket.socket.connect = denied
    socket.socket.connect_ex = denied
    socket.create_connection = denied
    socket.getaddrinfo = denied
    os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', HF_HUB_DISABLE_TELEMETRY='1')


def synthesize(payload):
    request = validate(payload)
    disable_network()
    # All imports and upstream diagnostic output stay away from protocol files.
    import numpy as np
    import torch
    import spacy
    import spacy.cli
    def no_download(*args, **kwargs):
        raise RuntimeError('automatic asset installation disabled; install English G2P assets explicitly')
    spacy.cli.download = no_download
    # Fail before Misaki can invoke spaCy's package installer.
    if not spacy.util.is_package('en_core_web_sm'):
        raise RuntimeError('missing local en_core_web_sm; install compatible English G2P assets explicitly')
    from kokoro import KModel, KPipeline
    model = KModel(repo_id='hexgrad/Kokoro-82M', config=payload['config_path'], model=payload['model_path']).to('cpu').eval()
    pipeline = KPipeline(lang_code={'en-us': 'a', 'en-gb': 'b'}[request['voice']['language']],
                         repo_id='hexgrad/Kokoro-82M', model=model, device='cpu')
    voice = torch.load(payload['voice_path'], map_location='cpu', weights_only=True)
    if not isinstance(voice, torch.Tensor) or voice.dtype != torch.float32:
        raise ValueError('voice asset must be a float32 tensor')
    chunks, phonemes = [], []
    with torch.inference_mode():
        for result in pipeline(request['text'], voice=voice, speed=request['voice']['speed']):
            audio = result.audio
            if audio is None:
                raise ValueError('Kokoro returned no audio')
            chunks.append(audio.detach().cpu().numpy().reshape(-1))
            phonemes.append(result.phonemes)
    if not chunks:
        raise ValueError('Kokoro returned empty audio')
    audio = np.concatenate(chunks)
    if not len(audio) or not np.isfinite(audio).all():
        raise ValueError('Kokoro returned empty or nonfinite audio')
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype('<i2')
    with wave.open(payload['output_path'], 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(24000)
        wav.writeframes(pcm.tobytes())
    return {'ok': True, 'protocol': 1, 'sample_rate': 24000, 'sample_count': len(audio),
            'phonemes': ' '.join(phonemes), 'alignment': 'not_aligned',
            'warnings': ['Speaking style is metadata only; no emotion conditioning is applied.'] if request['voice'].get('style') else []}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--request', required=True, type=Path)
    parser.add_argument('--response', required=True, type=Path)
    args = parser.parse_args()
    code = 0
    try:
        if args.request.stat().st_size > 100000:
            raise ValueError('request too large')
        payload = json.loads(args.request.read_text(encoding='utf-8'))
        with contextlib.redirect_stdout(sys.stderr):
            response = synthesize(payload)
    except Exception as exc:
        response = {'ok': False, 'protocol': 1, 'error': f'{type(exc).__name__}: {exc}'}
        code = 1
    temp = args.response.with_suffix('.tmp')
    temp.write_text(json.dumps(response, allow_nan=False), encoding='utf-8')
    temp.replace(args.response)
    return code


if __name__ == '__main__':
    raise SystemExit(main())
