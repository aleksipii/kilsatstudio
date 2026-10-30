//! Continuity notes on the checked-in shot-list example.
use motion_script::{expand_shots, ShotList};
use std::path::PathBuf;

#[test]
fn night_room_reference_expands_without_continuity_notes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/night-room-reference");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    let base = motion_script::parse(&read("base.yaml")).unwrap();
    let list = ShotList::from_yaml(&read("shots.yaml")).unwrap();
    let expansion = expand_shots(&base, &list).unwrap();
    // Every shot is a single of one character with the other off stage, so no
    // screen order is established; framings alternate between characters and
    // no move or gesture starts near a cut.
    assert_eq!(expansion.continuity, vec![]);
}
