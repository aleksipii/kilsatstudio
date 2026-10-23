# ScriptIntent v1 intake (INT-01, first slice)

Status: **implemented as a library module and CLI example; not wired into the
desktop editor.** This is the first item of the integration order in
[docs/automation/agents/10-integration.md](automation/agents/10-integration.md)
(`INT-01`, owned by `motion-script`), covering rules ING-002, ING-004 (bounded
vocabulary only) and ING-005 of [01-script](automation/agents/01-script.md).

Code: `crates/motion-script/src/intent.rs` (module `motion_script::intent`),
tests in `crates/motion-script/src/intent_tests.rs`. It is a module of the
existing crate rather than a new crate because the plan places the intent
contracts and adapter diagnostics under `motion-script` and the next step
(ING-006) must call the existing `parse`/`validate`/`expand_rules`. It does not
parse YAML, own a clock or evaluate scenes.

```sh
cargo run -p motion-script --example script_intent -- \
    examples/automation/pilot-fi.txt --language fi-FI --seed 31 --scene-id varikko
cargo run -p motion-script --example script_intent -- \
    examples/automation/pilot-en.txt --language en-GB --seed 31 --scene-id varikko
```

## Accepted text format

```text
<title line>
Kesto: 50 sekuntia. Paikka: <location>.         | Duration: 50 seconds. Location: <location>.
Hahmot: Aino, <role>; Rane, <role>.             | Characters: Aino, <role>; Rane, <role>.
[0–3] <action text>
[3–8] RANE: <dialogue exactly as spoken>
```

- Header keywords must match the selected language family; `fi-FI` text run as
  `en-GB` is `LanguageMismatch`. `en-US` vs `en-GB` is never inferred: language,
  seed and story-scene id are explicit options because the text does not carry them.
- 1–2 characters, one capitalised word each (Unicode letters or `-`); the id is the
  upper-cased name (`Äijä` → `ÄIJÄ`). Roles are kept in the returned header, not
  in the intent (the contract has no role field).
- Times are scene-local seconds, `digits[.ddd]`, half-open `[start,end)`, `–` or `-`.
  Duration 30–60 s. Signs, exponents, `NaN` and `inf` cannot be written.
- A line is dialogue when the text before the first `:` is an upper-case label.
  Dialogue keeps its exact text; the span excludes the label and surrounding
  whitespace. Other timed lines are action lines.
- Blank lines and CRLF are accepted. Continuation lines and `SCENE`/`KOHTAUS`
  markers are reported as unsupported (one story scene per file in this slice).

## Output

`ScriptIntent` serializes in the field order of the committed fixtures:
`version, seed, language, duration_s, source_sha256, characters, scenes, beats,
unresolved, audio_status, render_status, speech_language, caption_language`.
Dialogue beats carry `text`, `source_text`, a UTF-8 `source_span` and
`provenance {kind: authored, rule_id: ING-002, rule_pack: proposal-v1}`.
`ScriptIntent::verify_against` rejects a changed source (hash or span text).

Action lines are interpreted only when the whole line is one sentence from the
ING-004 vocabulary: `X katsoo Y`/`X looks at Y` (Y a declared character; Finnish
partitive `-a/-ä` accepted) → `look_at`; `X näyttää puhelimen`/`X shows the|his|her|their phone`
→ `present_prop` target `phone`; `Tauko`/`Pause` → `hold`. These beats are
`derived` with rule `ING-004`. Every other action line is preserved verbatim in
`unresolved` as `unsupported_action` / `needs_review` at its span. A timeline that
ends before the declared duration adds `timeline_underfilled` (ING-005) at the
duration number. All three pilot action lines (including the Kilsat app-screen
line) end up in `unresolved`; the intake does not invent, render or describe the
app screen.

Errors are typed (`IntakeErrorKind`, with `category()` = syntax / invalid
reference / unsupported / bound / invalid) and carry byte spans. Header errors
stop intake; every timed-line error is reported together. Covered: unknown
speaker (no character is added), upper-case label without colon, lower-case
`Rane:` label (ambiguous, not guessed), empty text, backwards/zero interval,
out-of-order start, line past duration, same-speaker overlap, different-speaker
overlap (unsupported), third character, duplicate character, invalid number,
source > 64 KiB, > 64 timed lines, > 1000 dialogue characters, invalid UTF-8,
invalid scene id. Actions may overlap dialogue.

## Fixture comparison and contract discrepancies

Tests compare the produced JSON with `examples/automation/pilot-intent.json` and
`pilot-en-intent.json` **field for field** (as JSON values) and pass, with these
explicit, documented differences. The committed fixtures were not changed.

1. `shot_proposal` and `app_insert` are in the fixtures but are direction/camera
   decisions (02/05, DIR-08). The intake does not produce them; the test removes
   them from the expected side.
2. `unresolved` is produced by intake but is absent from the fixtures.
   10-integration assigns "unresolved points" to `ScriptIntent`, and the fixtures
   silently drop the three action lines, so the field is added rather than the
   action lines being lost. The test removes it from the produced side.
3. Scene id `varikko` cannot be derived from either location text, so it is an
   explicit option.
4. The fixtures contain no action beats. `tools/check_animation_plan.py` accepts
   only `provenance.kind == "authored"` beats, while 01-script requires
   `derived` for inferred decisions; an intent with a resolved ING-004 action
   beat would therefore fail the current checker. The pilots produce none, so
   the checker is unchanged.
5. Byte-for-byte equality is not claimed: (1)–(2) make it impossible.

## Not done

- ING-001 (YAML passthrough is the existing `parse`), ING-003 free prose,
  multi-scene markers, multi-line dialogue, locks.
- ING-006 intent → MotionScript: not drafted. 01-script requires that a missing
  asset blocks compilation, and both pilots have `asset_status: to_be_authored`,
  so a MotionScript draft would contradict the contract.
- Editor review UI, persistence of spans, the 5 s ingest budget measurement on a
  target Mac, `AssetManifest`, `DirectionPlan` and everything after INT-01.
