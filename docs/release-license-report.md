# Offline release and Mac evidence reports

The new collectors read explicitly supplied local evidence. They never download,
run Cargo/pip, load models, execute the application or grant distribution/device
approval. All tests use synthetic evidence; no upstream license text was added.

## License corpus collector

```sh
python3 scripts/release_license_report.py --archive /local/preview.zip \
  --audit /local/audit.json --build-evidence /local/build.json \
  --corpus-index /local/corpus-index.json --corpus-root /local/corpus \
  --repo /local/matching-checkout --output /local/new-license-report.json --strict
```

Supply canonical absolute input/root/output-parent paths. On macOS, resolve trusted
OS aliases such as `/var` → `/private/var` before invoking these tools (for example,
`Path(trusted_path).resolve()`). The collectors deliberately reject symlink
components within supplied evidence paths; they do not silently resolve untrusted
corpus references.

The checkout must match the packaged resources. The collector reruns the trusted
adjacent package auditor and requires an exactly matching supplied audit. Build
artifact, executable observations and audit-byte identity must agree. Desktop and
GPU helper graphs remain separate. This first version requires both executables.
Caller-supplied build graphs are observations, not independent compiler attestation;
`raw-build-input-attestation-not-established` deliberately remains unresolved.

The schema-1 corpus index has `artifact` equal to the audited artifact object,
`build_evidence` containing the build report's `sha256` and `size_bytes`, and arrays
`components`, `evidence`, `reviews`. A component has a unique `id`, `name`,
`version`, exact `source`, `revision`, and `boundary` (`bundled`, `local-install`,
`external`). Optional `graph_refs` contain `graph` (`studio-desktop` or
`motion-render`) and exact package `id`. Name/version/source must match; graph
candidates remain bundled conservative candidates rather than disappearing through
an external label. Unmapped candidates are reported. Optional `parent` associates
embedded font/data/native subcomponents; cycles and unknown parents are invalid.
Optional `members` bind canonical ZIP `path` and `sha256`. External components
cannot claim bundled members. `embedded_inventory_complete` is a reviewer claim,
not a scan result; absent claims remain gaps.

Evidence entries contain unique `id`, `component`, canonical corpus-relative
`path`, `source` matching the component source, `kind`, `sha256`, `size_bytes`.
Kinds are license, notice, copyright, source, modification and provenance. Files
are hashed without extraction/execution. The supplied corpus directory is bounded
to 4096 entries, 1 MiB per file and 32 MiB total. Symlinks, special files,
traversal, duplicate references and evidence hardlink aliases are rejected. JSON
inputs/reports are bounded to 16 MiB. Do not put model/wheel payloads in this text
corpus; retain their artifact identities separately.

Reviews contain `component`, `reviewer`, `date` (YYYY-MM-DD), `rationale`,
`declared_expression`, optional `selected_alternative`, `component_sha256`,
`evidence`, `obligations`. Compute the component digest over UTF-8
`json.dumps(component, sort_keys=True, separators=(',', ':'))`. Review evidence
entries bind each component evidence `id` and `sha256`; stale or partial bindings
are invalid. License choices are recorded, never inferred or legally validated.

Obligations must address all categories: `notice-copyright`, `modification`,
`source`, `asset-provenance`. Each has `status` (`unreviewed`, `required`,
`satisfied`, `reviewed-not-applicable`), `rationale` and supporting evidence IDs
in `evidence`. Missing categories/support remain unresolved. Applicable notice
obligations also contain future ZIP `destination` and exact text `sha256`;
prepared bytes absent from the audited ZIP are `notice-not-packaged`. This version
does not change the packager allowlist or insert third-party notices.

## Mac observation collector

```sh
python3 scripts/mac_acceptance_report.py --archive /local/preview.zip \
  --observations /local/operator.json --evidence-root /local/evidence \
  --output /local/new-device-report.json --strict
```

Schema-1 operator input has `artifact` (`sha256`, `size_bytes` of the exact ZIP),
`operator`, `utc_date` (YYYY-MM-DD in UTC), `mac_model`, `chip`, `macos`, full `build_commit`, and
`observations`. Each observation has `gate`, `status` (`pass`, `fail`,
`not_tested`), `steps`, `expected`, `actual`, and `evidence` entries binding
relative `path`, `sha256`, `size_bytes`. Tested gates require retained evidence.
Missing gates remain not tested. Authority review and independent binding of the
operator's commit claim to ZIP contents always remain explicit gaps; even all-pass
claims cannot make strict collection succeed. Gate names are `launch-project`,
`offline-kokoro`, `audio-devices`, `seek-pause-sync`, `real-speech-video`,
`finder-events`, `shortcuts-accessibility`, `full-workflow`. Follow the detailed
[physical Mac procedure](workstreams/next-mac-device.md); a label and file hash do
not prove that procedure was performed. At most 64 observations, 128 evidence
references per observation, 16 MiB per file and 128 MiB total are read. Keep large
recordings separately; provide bounded human reports with their recording hashes.

Both collectors publish new reports atomically without replacing previous files.
Exit 0 means collection succeeded, strict exit 1 means gaps remain, exit 2 means
invalid inputs/output failure. Every report always has
`distribution_approved=false`, `physical_device_acceptance=not_granted`.
Even eight supplied pass claims require separate authority review; the collector
cannot authenticate the operator, hardware, upstream publisher or legal judgment.

Remaining external inputs are the complete exact upstream license/NOTICE/source
corpus, qualified rights review, packaged obligations, production signing/channel
decision and retained physical Mac observations. No CI result closes those gates.
