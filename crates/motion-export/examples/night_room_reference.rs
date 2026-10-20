//! Night-room reference sequence helper.
//!
//! ```sh
//! cargo run -p motion-export --example night_room_reference            # timing report
//! cargo run -p motion-export --example night_room_reference -- write   # refresh files
//! ```
//!
//! `report` prints each shot's start/end and the cut points on the 24 kHz
//! sequence clock. `write` regenerates `night-room.kilsat` and the placeholder
//! `audio/*.wav` fixtures in `examples/night-room-reference/`.
#[path = "../tests/support/mod.rs"]
mod support;

fn main() {
    match std::env::args().nth(1).as_deref() {
        None | Some("report") => print!("{}", support::timing_report(&support::expansion())),
        Some("write") => {
            support::write_all(&support::example_dir());
            println!("wrote {}", support::example_dir().display());
        }
        Some(other) => {
            eprintln!("unknown command {other}; use `report` or `write`");
            std::process::exit(2);
        }
    }
}
