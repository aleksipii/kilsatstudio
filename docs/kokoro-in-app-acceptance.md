# In-app Kokoro acceptance checklist

This checklist separates implementation checks from tests that must run on a real
Mac. An installed profile or a passing prerequisite check does not establish that
Kokoro can synthesize speech. None of the unchecked device tests below has been
performed by Linux CI.

## Automated checks

Record exact commands and results in the pull request. Tests use temporary local
fixtures or fake subprocesses: they must not install pip packages, download model
assets, or claim real inference.

- [ ] Missing download consent rejects setup before creating directories, invoking
  pip, or opening network connections.
- [ ] The shipped manifest pins an immutable model revision, exact asset byte
  sizes and full SHA256 values. Corrupt, truncated and oversized downloads fail.
- [ ] Required model/config/English voice files and an English language resource
  are declared. Direct package versions are pinned; this is not a hash-locked
  transitive environment or completed redistribution audit.
- [ ] Existing destinations are never overwritten. Failure cleanup removes only
  the newly created installation root owned by that attempt.
- [ ] The virtual environment is created in its final location; moving it after
  creation must not break interpreter paths or generated script shebangs.
- [ ] Installer output, diagnostics, queues and subprocess lifetimes are bounded.
  Cancel, timeout and application shutdown stop descendant processes and reap
  direct children. A retry can succeed after a failed or cancelled attempt.
- [ ] Profile parsing validates the schema, required files and cache directory.
  Large model hashing and subprocess work run outside the UI event loop.
- [ ] A speech test uses a dedicated short request and cache, does not rewrite the
  screenplay, and reports synthesis separately from playback-device failures.
- [ ] Test playback follows the existing audio source sample clock. Its position
  is not described as measured hardware output latency or validated lip sync.

## Real Mac prerequisite acceptance

Use a native Apple Silicon Mac with a supported macOS version and Python
3.10–3.12. Keep the application version/commit, macOS and CPU architecture, Python
version, selected manifest revision, setup result and resolved dependency inventory
with the test evidence. Do not place downloaded model assets in Git.

- [ ] Open the packaged application and confirm the Kokoro setup page explains
  network downloads and disk use before explicit consent.
- [ ] Decline consent: no installation starts. Grant consent and install into a
  new destination. Confirm the window remains responsive throughout.
- [ ] Cancel during asset transfer and package installation; confirm no setup
  processes remain, the attempt's files are cleaned up, and retry works.
- [ ] Restart the app and reload the installed profile. Confirm paths still point
  to the final environment and no setup download happens automatically.
- [ ] Disconnect external networking and run the local prerequisite check.
  Save its report; a pass still does not establish real speech generation.

## Real CPU speech and audio acceptance

- [ ] With external networking disabled, generate a short English test sentence.
  Confirm it produces a nonempty, finite, non-silent WAV using real Kokoro CPU
  inference, rather than the synthetic sound-check demo.
- [ ] Listen to the WAV through the Mac's selected output device. Record whether
  words are intelligible and whether there are clipping, truncation or device
  errors. A successful WAV alone is not successful device playback.
- [ ] Test `af_heart` and `am_michael` and at least two supported speed values.
  Verify switching voice/settings regenerates or selects the correct cache entry.
- [ ] Stop/cancel a running test, retry, disconnect/reconnect an output device and
  retry playback. Confirm actionable errors and a responsive window.
- [ ] Prepare and play a real project scene with dialogue. Check pause, seek,
  restart, scene boundaries and exported audio. Confirm standalone test audio did
  not alter the project's source, Undo history, scene selection or prepared clips.
- [ ] Measure alignment separately against real recorded audio before claiming
  perceptual mouth timing quality. Kokoro phoneme tokens are not timestamps.

## Recorded Mac evidence

| Date | Machine and build | Result reported by the user | Not covered |
| --- | --- | --- | --- |
| 2026-10-16 | Apple M1, macOS 27.1; preview artifact from `main` at 98d265b (macOS practical preview run #61); Homebrew Python 3.12 | Managed in-app setup installed and connected a profile. The in-app English CPU speech test produced audio for `af_heart` and `am_michael`, and the user reported that Kokoro works after listening to both. | Offline run with networking disabled, consent decline, cancel/retry, restart/reload, multiple speeds, clipping/truncation notes, device switching and full scene playback. No WAV, JSON evidence report or BUILD-INFO.txt was attached. |

The checklist items above stay unchecked until each one is exercised as written.
This entry is the first real Apple Silicon synthesis report; it does not establish
offline repeatability or the Speech 2 gate.

The setup flow remains a local Python CPU sidecar. This checklist does not claim
native Rust inference, Metal/MPS/Core ML acceleration, notarization, a complete
Python wheel lock, or clearance to redistribute every dependency and voice asset.
