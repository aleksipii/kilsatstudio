//! The same screenplay must feed the independent scene and speech pipelines.
#[test]
fn movement_and_dialogue_share_actor_scene_and_timeline() {
    let script = motion_script::parse(include_str!("../../../examples/hello-emma.yaml")).unwrap();
    let jobs = motion_voice::dialogue_jobs(&script, std::path::Path::new(".")).unwrap();
    assert_eq!(
        jobs.len(),
        2,
        "non-dialogue events must never synthesize speech"
    );
    assert_eq!(jobs[0].actor, "EMMA");
    assert_eq!(jobs[0].at, 3.0);
    assert_eq!(jobs[0].request.voice.preset, "af_heart");
    assert_eq!(jobs[1].request.voice.preset, "am_michael");
    let evaluated = motion_scene::evaluate(&script, 0, jobs[0].at).unwrap();
    let emma = evaluated
        .actors
        .iter()
        .find(|a| a.id == jobs[0].actor)
        .unwrap();
    assert_eq!(emma.x, 500.0);
    assert_eq!(emma.y, 740.0);
    assert_eq!(emma.emotion, "happy");
    let later = motion_scene::evaluate(&script, 0, 10.0).unwrap();
    assert_eq!(
        later.actors.iter().find(|a| a.id == "EMMA").unwrap().x,
        850.0
    );
}
