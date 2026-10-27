# EventLog v1 examples

Event logs for the screenplay used in `crates/motion-script/tests/event_log.rs`
(scenes `one` and `two`, humanoid actors `A`/`B`, custom-rig actor `C`).
Format: [animation rules § EventLog v1](../../docs/animation-rules.md#eventlog-v1).

| File | Expected result |
| --- | --- |
| `valid.json` | validates against the test screenplay; same log as `ok_sorted()` in the tests |
| `invalid-order.json` | `EventLogError::Order` at `events[1].sample` (scene `one` goes back in time) |
