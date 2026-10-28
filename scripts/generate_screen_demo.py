#!/usr/bin/env python3
"""Author the "Live Demo" example episode: Aino plays a recording on her phone.

Writes examples/screen-demo/live-demo.kilsat and, with --ffmpeg, the original
illustrative app recording assets/kilsat-app-demo.mp4 that plays in the
phone's screen slot. The recording is drawn here from geometric lettering; it
is marked EXAMPLE and is not a capture of a real app.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import subprocess

import generate_kilsat_season as original

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'examples/screen-demo'
VIDEO = 'assets/kilsat-app-demo.mp4'
VIDEO_W, VIDEO_H, VIDEO_FPS, VIDEO_SECONDS = 540, 1170, 30, 8.0
VOICES = {'AINO': ('af_heart', 1.0), 'RANE': ('am_michael', 0.97)}
# Aino holds the phone in her right hand, arm raised toward the camera.
ARM = -1.9
PHONE_CENTER = (0.0, 80.0)  # in arm_r space, at the hand
PHONE_W, PHONE_H, BEZEL = 30.0, 58.0, 2.4

TURNS = [
    ('AINO', "Watch this. Kilsat logs my drive while I'm still parking."),
    ('RANE', 'I log my drives too. Mentally. With great confidence.'),
    ('AINO', 'Mine shows every trip, the distance, and what I can claim.'),
    ('RANE', 'Numbers on a phone. Very brave. Very small.'),
    ('AINO', "Here. Today's trips, saved before the engine cooled down."),
    ('RANE', "Thirty-eight kilometers? I'd have rounded that up to a hundred."),
    ('AINO', 'That is why your log gets audited and mine gets paid.'),
    ('RANE', 'Send me the app. Purely to check that it is wrong.'),
]

GLYPHS = dict(original.FONT)
GLYPHS.update({
    'B': ['11110', '10001', '10001', '11110', '10001', '10001', '11110'],
    'F': ['11111', '10000', '10000', '11110', '10000', '10000', '10000'],
    'W': ['10001', '10001', '10001', '10101', '10101', '11011', '10001'],
    '.': ['00000', '00000', '00000', '00000', '00000', '01100', '01100'],
    '>': ['01000', '00100', '00010', '00001', '00010', '00100', '01000'],
    '+': ['00000', '00100', '00100', '11111', '00100', '00100', '00000'],
})


def app_frame(t: float) -> bytes:
    """One RGB frame of the illustrative mileage-app recording at time t."""
    w, h = VIDEO_W, VIDEO_H
    pixels = bytearray(bytes((19, 50, 43)) * (w * h))

    def rect(x, y, rw, rh, color):
        x0, x1 = max(0, int(x)), min(w, int(x + rw))
        if x1 <= x0:
            return
        row = bytes(color) * (x1 - x0)
        for yy in range(max(0, int(y)), min(h, int(y + rh))):
            pixels[(yy * w + x0) * 3:(yy * w + x1) * 3] = row

    def text(value, x, y, scale, color):
        for c in value:
            for yy, bits in enumerate(GLYPHS[c]):
                for xx, bit in enumerate(bits):
                    if bit == '1':
                        rect(x + xx * scale, y + yy * scale, scale, scale, color)
            x += 6 * scale

    cream, mint, gold, dark = (242, 239, 224), (91, 224, 193), (255, 223, 124), (19, 50, 43)
    rect(0, 0, w, 170, (31, 92, 77))
    text('KILSAT', 40, 64, 9, cream)
    rect(372, 76, 136, 46, gold)
    text('EXAMPLE', 381, 89, 3, dark)
    text('TODAY', 40, 222, 5, (156, 199, 187))
    distance = min(t * 6.4, 38.4)
    text(f'{distance:4.1f} KM'.strip(), 40, 296, 11, (255, 255, 255))
    rect(40, 410, 460, 18, (42, 77, 68))
    rect(40, 410, min(t / 6.0, 1.0) * 460, 18, mint)
    trips = [(1.2, 'HOME > OFFICE', '12.8'), (2.8, 'OFFICE > CLIENT', '25.6')]
    for index, (start, route, km) in enumerate(trips):
        if t < start:
            continue
        slide = min(1.0, (t - start) / 0.35)
        y = 480 + index * 140 + (1 - slide) * 40
        rect(40, y, 460, 116, (31, 74, 64))
        text(route, 64, y + 26, 4, (255, 255, 255))
        text(km + ' KM', 64, y + 70, 3, (156, 199, 187))
    if t >= 4.6:
        pop = min(1.0, (t - 4.6) / 0.25)
        inset = (1 - pop) * 30
        rect(40 + inset, 780 + inset, 460 - 2 * inset, 120 - 2 * inset, mint)
        if pop >= 1.0:
            text('SAVED  +7.30 EUR', 72, 826, 4, dark)
    # The tap: a ring where the finger presses "save", just before 4.6 s.
    if 4.25 <= t < 4.75:
        r = 26 + (t - 4.25) * 60
        for a in range(0, 360, 6):
            px = 270 + r * math.cos(math.radians(a))
            py = 840 + r * math.sin(math.radians(a))
            rect(px - 3, py - 3, 6, 6, (255, 255, 255))
    rect(170, 1110, 200, 12, (156, 199, 187))
    return bytes(pixels)


def write_video(ffmpeg: Path, destination: Path):
    destination.parent.mkdir(parents=True, exist_ok=True)
    frames = round(VIDEO_SECONDS * VIDEO_FPS)
    command = [str(ffmpeg), '-v', 'error', '-y', '-f', 'rawvideo', '-pix_fmt', 'rgb24',
               '-s', f'{VIDEO_W}x{VIDEO_H}', '-r', str(VIDEO_FPS), '-i', '-',
               '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '20', '-preset', 'slow',
               '-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709',
               '-movflags', '+faststart', '-fflags', '+bitexact', str(destination)]
    with subprocess.Popen(command, stdin=subprocess.PIPE) as process:
        for index in range(frames):
            process.stdin.write(app_frame(index / VIDEO_FPS))
        process.stdin.close()
        if process.wait() != 0:
            raise RuntimeError('ffmpeg failed to encode the demo recording')


def rotate(x, y, angle):
    c, s = math.cos(angle), math.sin(angle)
    return [round(x * c - y * s, 3), round(x * s + y * c, 3)]


def upright(dx, dy):
    """A point dx,dy from the phone centre, upright on screen while the arm is raised."""
    x, y = rotate(dx, dy, -ARM)
    return [round(PHONE_CENTER[0] + x, 3), round(PHONE_CENTER[1] + y, 3)]


def box(x0, y0, x1, y1, radius=0.0, steps=4):
    """Upright rectangle; rounded corners when radius > 0."""
    if radius <= 0:
        return [upright(x0, y0), upright(x1, y0), upright(x1, y1), upright(x0, y1)]
    points = []
    corners = [(x1 - radius, y0 + radius, -90), (x1 - radius, y1 - radius, 0),
               (x0 + radius, y1 - radius, 90), (x0 + radius, y0 + radius, 180)]
    for cx, cy, start in corners:
        for step in range(steps + 1):
            a = math.radians(start + 90 * step / steps)
            points.append(upright(cx + radius * math.cos(a), cy + radius * math.sin(a)))
    return points


def add_phone(rig):
    hw, hh = PHONE_W / 2, PHONE_H / 2
    gw, gh = hw - BEZEL, hh - BEZEL * 1.6
    edge = {'width': 1.4, 'mode': 'behind', 'color': [16, 18, 22, 255]}
    parts = [
        {'id': 'phone_body', 'joint': 'arm_r', 'points': box(-hw, -hh, hw, hh, radius=4.5),
         'color': [34, 37, 44, 255], 'outline': edge},
        {'id': 'phone_glass', 'joint': 'arm_r', 'points': box(-gw, -gh, gw, gh, radius=2.5),
         'color': [19, 50, 43, 255]},
        # Drawn lock screen; hidden while a recording plays in the slot.
        {'id': 'phone_clock', 'joint': 'arm_r', 'points': box(-7, -12, 7, -7),
         'color': [242, 239, 224, 255]},
        {'id': 'phone_reflection', 'joint': 'arm_r',
         'points': [upright(-gw, -gh), upright(-gw + 9, -gh), upright(-gw, -gh + 18)],
         'color': [255, 255, 255, 46]},
        {'id': 'thumb', 'joint': 'arm_r', 'ellipse': dict(zip(('cx', 'cy'), upright(-hw + 1, 9)), rx=5.5, ry=8),
         'color': [233, 184, 151, 255], 'outline': {'width': 1.4, 'mode': 'behind', 'color': [27, 28, 36, 255]}},
    ]
    names = [p['id'] for p in rig['parts']]
    at = names.index('hand_r') + 1
    rig['parts'][at:at] = parts
    origin = upright(-gw, -gh)
    rig['screens'] = [{
        'id': 'phone', 'part': 'phone_glass', 'origin': origin,
        'x_axis': [round(a - b, 3) for a, b in zip(upright(gw, -gh), origin)],
        'y_axis': [round(a - b, 3) for a, b in zip(upright(-gw, gh), origin)],
        'overlays': ['phone_clock'],
    }]


def screen(at, trim=0.0, taps=()):
    return {'actor': 'AINO', 'slot': 'phone', 'at': at,
            'source': {'type': 'video', 'asset': VIDEO, 'fps': VIDEO_FPS, 'duration': VIDEO_SECONDS},
            'fit': 'cover', 'trim_start': trim, 'trim_end': None, 'speed': 1.0, 'loop': False,
            'taps': list(taps)}


def hold_phone(comp, duration):
    """Keep Aino's phone up, with a slight bob so the hand feels alive."""
    comp['joint_tracks']['AINO']['arm_r'] = [
        original.key(0, rotation=ARM), original.key(duration / 2, rotation=ARM + 0.035),
        original.key(duration, rotation=ARM)]


def episode():
    scenes, art = [], []
    shown = False
    for index, (speaker, line) in enumerate(TURNS):
        identifier = f'demo_line{index + 1:02d}'
        duration = round(min(5.4, max(4.0, len(line) / 18.0 + 0.6)), 3)
        emotion = ('surprised' if index == 4 else 'happy') if speaker == 'AINO' else (
            'angry' if index in (5, 7) else 'happy')
        comp = original.composition(identifier, 'parkkipaikka', speaker, index, duration)
        hold_phone(comp, duration)
        if shown:
            # After the demo the phone keeps showing the saved trip.
            comp['screens'] = [screen(0, trim=VIDEO_SECONDS - 0.1)]
        scenes.append({'id': identifier, 'duration': duration, 'background': 'parkkipaikka',
                       'composition': comp,
                       'events': [{'type': 'emotion', 'at': 0, 'actor': speaker, 'value': emotion},
                                  {'type': 'speak', 'at': 0.35, 'actor': speaker, 'text': line,
                                   'emotion': emotion, 'audio': None}]})
        art.extend(original.scenery(identifier, 'parkkipaikka'))
        if index == 4:
            # Close-up: the recording plays in the phone's screen slot.
            insert, length = 'demo_phone_closeup', 6.4
            comp = original.composition(insert, 'parkkipaikka', 'AINO', 0, length)
            hold_phone(comp, length)
            comp['camera'] = [dict(at=0, x=-70, y=-30, zoom=3.2), dict(at=length, x=-75, y=-30, zoom=3.6)]
            comp['gaze_tracks']['AINO'] = [dict(at=0, x=900, y=520)]
            comp['screens'] = [screen(0.2, taps=[4.4])]
            scenes.append({'id': insert, 'duration': length, 'background': 'parkkipaikka',
                           'composition': comp, 'events': []})
            art.extend(original.scenery(insert, 'parkkipaikka'))
            shown = True
    reaction = 'demo_reaction'
    comp = original.composition(reaction, 'parkkipaikka', None, 8, 2.0)
    hold_phone(comp, 2.0)
    comp['screens'] = [screen(0, trim=VIDEO_SECONDS - 0.1)]
    comp['camera'] = [dict(at=0, x=0, y=-75, zoom=1.14)]
    comp['gaze_tracks']['RANE'] = [dict(at=0, target='AINO'), dict(at=0.7, x=960, y=40)]
    scenes.append({'id': reaction, 'duration': 2.0, 'background': 'parkkipaikka', 'composition': comp,
                   'events': [{'type': 'emotion', 'at': 0, 'actor': 'AINO', 'value': 'happy'},
                              {'type': 'emotion', 'at': 0, 'actor': 'RANE', 'value': 'angry'}]})
    art.extend(original.scenery(reaction, 'parkkipaikka'))
    characters = {name: original.character(name) for name in ('AINO', 'RANE')}
    for name, (preset, speed) in VOICES.items():
        characters[name]['voice'] = {'provider': 'kokoro', 'preset': preset, 'language': 'en-us',
                                     'speed': speed}
    add_phone(characters['AINO']['rig'])
    source = {'project': {'title': 'Kilsat EN - The Live Demo', 'width': original.WIDTH,
                          'height': original.HEIGHT, 'fps': original.FPS,
                          'captions': {'enabled': True, 'font_size': 54,
                                       'speaker_fill': {'AINO': '#ffdf7c', 'RANE': '#ffffff'},
                                       'outline_width': 4, 'bottom_margin': 0.035, 'max_width': 0.9,
                                       'line_height': 1.08, 'uppercase': False,
                                       'background': '#17232b'}},
              'characters': characters, 'scenes': scenes}
    return {'format_version': 2, 'source': json.dumps(source, ensure_ascii=False, separators=(',', ':')),
            'assets': [], 'art': {'layers': art}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=DEFAULT)
    parser.add_argument('--ffmpeg', type=Path, help='absolute ffmpeg; (re)encodes the app recording')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    if args.ffmpeg:
        write_video(args.ffmpeg, args.output / VIDEO)
    project = episode()
    (args.output / 'live-demo.kilsat').write_bytes(original.encode(project))
    seconds = sum(s['duration'] for s in json.loads(project['source'])['scenes'])
    print(json.dumps({'project': 'live-demo.kilsat', 'draft_seconds': round(seconds, 3),
                      'video': VIDEO if (args.output / VIDEO).exists() else None}))


if __name__ == '__main__':
    main()
