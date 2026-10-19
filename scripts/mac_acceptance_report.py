#!/usr/bin/env python3
"""Collect artifact-bound human Mac observations; never grant device acceptance."""
import argparse
from datetime import date
import json
import re
from pathlib import Path
import sys

try:
    from release_license_report import document, identity, publish, regular, relative, rows, text
except ImportError:
    import importlib.util
    spec = importlib.util.spec_from_file_location('release_license_report', Path(__file__).with_name('release_license_report.py'))
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    document, identity, publish, regular, relative, rows, text = [getattr(helper, x) for x in ('document', 'identity', 'publish', 'regular', 'relative', 'rows', 'text')]

GATES = ('launch-project', 'offline-kokoro', 'audio-devices', 'seek-pause-sync',
         'real-speech-video', 'finder-events', 'shortcuts-accessibility', 'full-workflow')
SCENARIOS = {
    'launch-project': ('launch', 'project-round-trip', 'invalid-project-recovery'),
    'offline-kokoro': ('uncached-network-disabled', 'voice-speed-variants', 'cache-reuse', 'cancel-retry'),
    'audio-devices': ('named-device-playback', 'device-change-recovery'),
    'seek-pause-sync': ('pause-resume', 'reverse-seek', 'scene-overlap', 'eof-restart', 'long-run-drift'),
    'real-speech-video': ('real-speech-export', 'physical-playback-sync', 'export-cancel'),
    'finder-events': ('double-click', 'open-with'),
    'shortcuts-accessibility': ('keyboard-workflow', 'accessibility-workflow', 'pressure-soft-mask-input'),
    'full-workflow': ('script-to-export', 'restart-recovery'),
}


def scenario_gaps(gate, evidence_paths):
    """Check procedure coverage, without authenticating any human claim."""
    required = SCENARIOS[gate['gate']]
    seen, gaps = set(), []
    for item in rows(gate.get('scenarios', []), 'device scenarios', 32):
        key = item.get('scenario')
        if key not in required or key in seen:
            raise ValueError('unknown/duplicate device scenario')
        seen.add(key)
        status = item.get('status')
        if status not in ('pass', 'fail', 'not_tested'):
            raise ValueError('invalid scenario status')
        for field in ('steps', 'expected', 'actual'):
            text(item.get(field), 'scenario ' + field)
        references = item.get('evidence_paths', [])
        if not isinstance(references, list) or len(references) > 128:
            raise ValueError('invalid scenario evidence paths')
        paths = [relative(path) for path in references]
        if len(set(paths)) != len(paths) or any(path not in evidence_paths for path in paths):
            raise ValueError('scenario evidence must reference unique retained gate evidence')
        if status in ('pass', 'fail') and not paths:
            raise ValueError('tested scenario requires retained evidence')
        if gate['status'] == 'not_tested' and status != 'not_tested':
            raise ValueError('untested gate cannot contain tested scenarios')
        if gate['status'] == 'pass' and status == 'fail':
            raise ValueError('passing gate cannot contain failed scenario')
        if status != 'pass':
            gaps.append(dict(gate=gate['gate'], scenario=key, reason=status))
    gaps += [dict(gate=gate['gate'], scenario=key, reason='not_tested')
             for key in required if key not in seen]
    return gaps


def collect(archive, observations, evidence_root):
    value, binding = document(observations)
    archive_id = identity(regular(archive, 128 * 1024 * 1024))
    if value.get('artifact') != archive_id:
        raise ValueError('observations do not bind current archive')
    for key in ('operator', 'utc_date', 'mac_model', 'chip', 'macos', 'build_commit'):
        text(value.get(key), key)
    date.fromisoformat(value['utc_date'])
    if not re.fullmatch('[0-9a-f]{40}', value['build_commit']):
        raise ValueError('build commit must be full lowercase SHA')
    root = Path(evidence_root)
    if any(x.is_symlink() for x in [root, *root.parents]) or not root.is_dir():
        raise ValueError('evidence root must be nonsymlink directory')
    input_inodes = {(Path(p).stat().st_dev, Path(p).stat().st_ino) for p in (archive, observations)}
    gates, seen, total, coverage_gaps = [], set(), 0, []
    for gate in rows(value.get('observations'), 'observations', 64):
        key = gate.get('gate')
        if key not in GATES or key in seen:
            raise ValueError('unknown/duplicate device gate')
        seen.add(key)
        if gate.get('status') not in ('pass', 'fail', 'not_tested'):
            raise ValueError('invalid observation status')
        for field in ('steps', 'expected', 'actual'):
            text(gate.get(field), field)
        evidence = rows(gate.get('evidence', []), 'device evidence', 128)
        if gate['status'] in ('pass', 'fail') and not evidence:
            raise ValueError('tested observation requires retained evidence')
        paths = set()
        for item in evidence:
            path = relative(item.get('path'))
            if path in paths:
                raise ValueError('duplicate device evidence')
            paths.add(path)
            info = (root / path).stat()
            if (info.st_dev, info.st_ino) in input_inodes:
                raise ValueError('device evidence aliases archive/observations input')
            data = regular(root / path, 16 * 1024 * 1024)
            total += len(data)
            if total > 128 * 1024 * 1024:
                raise ValueError('device evidence total budget exceeded')
            if any(item.get(k) != v for k, v in identity(data).items()):
                raise ValueError('device evidence digest/size mismatch')
        coverage_gaps.extend(scenario_gaps(gate, paths))
        gates.append(gate)
    unresolved = [dict(gate='authority-review', reason='not_granted'),
                  dict(gate='build-attestation', reason='operator commit claim not independently bound to ZIP contents')]
    unresolved += [dict(gate=key, reason='not_tested') for key in GATES if key not in seen]
    unresolved += [dict(gate=x['gate'], reason=x['status']) for x in gates if x['status'] != 'pass']
    unresolved += coverage_gaps
    return dict(schema_version=1, artifact=archive_id, input_observations=binding,
                distribution_approved=False, physical_device_acceptance='not_granted',
                operator_claims={k: value[k] for k in ('operator', 'utc_date', 'mac_model', 'chip', 'macos', 'build_commit')},
                required_scenarios=SCENARIOS,
                observations=sorted(gates, key=lambda x: x['gate']),
                unresolved=sorted(unresolved, key=lambda x: (x['gate'], x.get('scenario', ''))),
                trust='Human supplied observations, not machine or operator attestation. Final acceptance requires separate authority review.')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('archive', 'observations', 'evidence-root', 'output'):
        parser.add_argument('--' + flag, type=Path, required=True)
    parser.add_argument('--strict', action='store_true')
    args = parser.parse_args(argv)
    try:
        report = collect(args.archive, args.observations, args.evidence_root)
        publish(report, args.output)
        return 1 if args.strict and report['unresolved'] else 0
    except (OSError, ValueError, KeyError, TypeError, RecursionError) as error:
        print('Mac acceptance report failed: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
