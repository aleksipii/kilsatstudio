# Packaged Metal device evidence

The Apple Silicon preview includes an independently signed native `gpu_probe`
helper. The application does not load its optional wgpu dependency in the editor.
The explicit **Run Metal device check** button launches the bundled helper in a
single background worker. Nothing runs merely by opening this panel.

On a physical Mac, open the packaged preview, expand **Metal device check** and
enter a new absolute directory whose parent already exists, for example
`/Users/yourname/Desktop/kilsat-metal-evidence-01`. Click the run button. Existing
output is refused. Keep the package's BUILD-INFO.txt with the evidence and record
macOS version, Mac model, chip and whether the application is running natively.
The report identifies the adapter, backend and hardware device type. It is
accepted by the panel only for Metal IntegratedGpu/DiscreteGpu, the exact fixture
schema, expected dimensions/format and successful interior/validation checks.
Raw RGBA is displayed after bounded report/image reads; software/unknown adapters
are rejected. Preserve `report.json` and `fixture.rgba`; on Mac record hashes with
`shasum -a 256 report.json fixture.rgba` from the output directory.

The child has no standard input and its stdout/stderr are discarded, so logs
cannot exhaust editor memory. A 30-second process watchdog and Cancel kill and
reap the child. Partial output is retained for inspection because the editor does
not assume ownership of arbitrary directories. The helper itself removes only
its own newly created output if writing fails. Cancel/timeout never records a
successful result. Repeating a check requires a new output directory.

This is a real hardware **fixture** gate, not full evaluated-scene GPU parity,
production backend selection, performance evidence or physical acceptance of
speech/audio timing. A successful CI compile/package does not pass this gate.
Without a packaged Mac helper the panel explains how to obtain one and disables
the run action. Developers can omit `--gpu-probe` when packaging older CPU-only
previews; the resulting app remains usable.

CI builds the helper for aarch64-apple-darwin, verifies arm64 architecture and
system-only dynamic-library dependencies, signs the helper before signing the
outer app, verifies signatures and checks its archive executable permissions.
CI does not execute this hardware probe implicitly.
