//! Measure rule-expansion cost for rules × events (A03).
//!
//! Run: `cargo run --release -p motion-script --example rule_expansion_cost`
//!
//! Each case expands one scene. Timings are the median wall-clock time of
//! repeated full calls, including validation of the expanded script; the
//! `0 rules` row is that validation-only baseline. Cases:
//! - `scan`: emotion rules whose value never matches (trigger matching only);
//! - `condition`: emotion rules that match every event but whose emotion
//!   condition never holds, so each firing scans the scene's emotion history
//!   (worst case, quadratic in events);
//! - `logged`: key rules that never match, over a log of key presses.
use motion_script::{parse, EventLog, EventSource, ExternalEvent, LoggedEvent, Script};
use std::time::{Duration, Instant};

fn script(rules: usize, events: usize, case: &str) -> Script {
    let mut source = String::from(
        "animation_clips:\n  c: {duration: 1, bone_tracks: {head: [{at: 0, x: 0}, {at: 1, x: 1}]}}\n\
         characters:\n  A: {asset: a}\nscenes:\n  - id: s\n    duration: 10000\n    events:\n",
    );
    let authored = if case == "logged" { 0 } else { events };
    for i in 0..authored {
        source += &format!("      - {{type: emotion, at: {i}, actor: A, value: happy}}\n");
    }
    if authored == 0 {
        source += "      []\n";
    }
    if rules > 0 {
        source += "rules:\n";
    }
    for r in 0..rules {
        let body = match case {
            "scan" => "on: emotion, value: sad",
            "condition" => "on: emotion, when: {emotion: sad}",
            _ => "on: key, value: never, when: {actor: A}",
        };
        source += &format!("  r{r:02}: {{{body}, then: {{type: clip, clip: c, duration: 1}}}}\n");
    }
    parse(&source).expect("benchmark script parses")
}

fn log(events: usize) -> EventLog {
    EventLog {
        events: (0..events)
            .map(|i| LoggedEvent {
                scene: "s".into(),
                sample: i as u64 * 24_000,
                source: EventSource::Keyboard,
                value: ExternalEvent::Key { key: "k".into() },
            })
            .collect(),
        ..EventLog::new(24_000)
    }
}

fn median(mut run: impl FnMut()) -> Duration {
    run();
    let start = Instant::now();
    let mut samples = Vec::new();
    while samples.len() < 9 || (start.elapsed() < Duration::from_millis(300) && samples.len() < 201)
    {
        let t = Instant::now();
        run();
        samples.push(t.elapsed());
    }
    samples.sort();
    samples[samples.len() / 2]
}

fn main() {
    println!("| case | rules | events | median µs |");
    println!("| --- | ---: | ---: | ---: |");
    for case in ["scan", "condition", "logged"] {
        for events in [16, 128, 1024] {
            for rules in [0, 1, 8, 64] {
                let script = script(rules, events, case);
                let log = log(if case == "logged" { events } else { 0 });
                let time = median(|| {
                    let expansion = script.expand_rules_with_events(&log).expect("expands");
                    assert!(expansion.firings.is_empty());
                    std::hint::black_box(expansion);
                });
                println!(
                    "| {case} | {rules} | {events} | {:.1} |",
                    time.as_secs_f64() * 1e6
                );
            }
        }
    }
}
