# Remaining Mac and release work

Updated 2026-10-19. Owner: `remaining_release_mac`.
Requirements and completion authority: [master progress](../master-progress.md).
This workstream separates implemented evidence checks from physical tests and
distribution authorization. No product milestone is accepted by this document.

## Implemented in this increment

- [x] Expand the offline Mac observation collector with named procedure coverage:
  launch/project recovery, uncached offline synthesis and voice/speed/cache/cancel,
  device playback/recovery, pause/seek/overlap/EOF/drift, speech export/playback,
  Finder events, keyboard/accessibility/pressure input, end-to-end and restart.
- [x] Bind tested scenario claims to retained, hash-checked gate evidence. Reject
  unknown/duplicate scenarios, absent/duplicate evidence references and conflicting
  gate/scenario status. Broad old pass labels retain explicit coverage gaps.
- [x] Keep `physical_device_acceptance=not_granted` and
  `distribution_approved=false`, including complete synthetic pass reports.
- [x] Nine focused Python tests pass locally, using synthetic files only.

See the [collector guide](../release-license-report.md) for the existing invocation
and the scenario format. `required_scenarios` in each new report gives the exact
checklist for that collector version. Archive identity, operator identity claims,
date and commit remain mandatory. These checks validate record completeness and
integrity; they do not authenticate a device, reviewer or observation.

## Physical Mac execution still required

- [ ] Retain the exact current preview ZIP, its SHA256 and BUILD-INFO; use a
  matching source checkout. Never combine observations from different builds.
- [ ] Run each required scenario on a named physical Mac and named audio device;
  record expected/actual behavior and retained evidence for each scenario.
- [ ] Test tablet/stylus/touch pressure and soft masks on real input hardware;
  backend event support is not proof that a particular Mac tablet works.
- [ ] Run the separate [painted-mask/Metal procedure](../painted-mask-mac-acceptance.md)
  and retain actual output, adapter identity and measured quality/performance.
  The procedure coverage report does not itself measure Metal parity.
- [ ] Execute offline Kokoro with already authorized local assets and networking
  disabled; retain new uncached WAVs and listening/synchronization observations.
- [ ] Obtain independent artifact/build binding and explicit acceptance review.

This environment has not exercised Finder, CoreAudio, a physical input device,
Metal, or real Kokoro synthesis. Operator scenarios and retained hashes are not
hardware attestation. A missing supported hardware path must be recorded as an
actual failure or `not_tested`, not relabeled passing.

## Release and license work still required

- [ ] Collect exact upstream license/NOTICE/source texts for every bundled
  dependency, embedded font/data/native component and separately installed asset.
- [ ] Obtain qualified component/asset rights review, choose any license
  alternatives explicitly and record source/modification/notice obligations.
- [ ] Prepare reviewed notice/source materials, extend package/audit boundaries
  together where necessary and re-audit the final byte-changing artifact.
- [ ] Decide production version/channel, minimum macOS, supported architectures,
  helper inclusion and Developer ID/notarization requirements. Signing credentials
  and legal decisions are external inputs, never test fixtures or repository data.
- [ ] Retain final build, signing/notarization (if selected), license and physical
  acceptance evidence together; obtain release authority approval for exact hashes.

The existing [release/license handoff](next-release-license.md) details collector
contracts and ordered packaging work. Its historical specification paragraphs do
not override the implemented collector described in the current
[report guide](../release-license-report.md). The root MIT license covers original
source only and does not close third-party distribution obligations.
