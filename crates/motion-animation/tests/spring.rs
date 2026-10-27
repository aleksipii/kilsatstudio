use motion_animation::spring::{
    MAX_DAMPING, MAX_FORCED_HORIZON, MAX_MASS, MAX_POSITION, MAX_SPRING_TIME, MAX_STIFFNESS,
    MAX_VELOCITY, MIN_COEFFICIENT,
};
use motion_animation::{
    release_state, DampingRegime, ForceProfile, ForcedSpring, Gust, ReleaseSample, Spring,
    SpringError, SpringParams, SpringProfile, SpringState, Turbulence,
};

fn params(m: f64, k: f64, c: f64) -> SpringParams {
    SpringParams::new(m, k, c).unwrap()
}

fn spring(p: SpringParams, x0: f64, v0: f64) -> Spring {
    Spring::from_velocity(p, 0.0, x0, v0).unwrap()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn profiles_match_report_table_and_regimes() {
    let expected = [
        (SpringProfile::Snappy, 1.00, DampingRegime::Critical),
        (SpringProfile::Gentle, 0.61, DampingRegime::Underdamped),
        (SpringProfile::Bouncy, 0.28, DampingRegime::Underdamped),
        (SpringProfile::Stiff, 1.00, DampingRegime::Critical),
        (SpringProfile::Wobbly, 0.19, DampingRegime::Underdamped),
    ];
    assert_eq!(SpringProfile::ALL.len(), expected.len());
    for (profile, zeta, regime) in expected {
        let p = profile.params();
        let (m, k, c) = profile.coefficients();
        assert_eq!(p, SpringParams::new(m, k, c).unwrap());
        assert_eq!(
            (p.damping_ratio() * 100.0).round() / 100.0,
            zeta,
            "{profile:?}"
        );
        assert_eq!(p.regime(), regime, "{profile:?}");
        assert_eq!(p.natural_frequency(), (k / m).sqrt());
    }
    assert_eq!(SpringProfile::Snappy.params().damping_ratio(), 1.0);
    assert_eq!(params(1.0, 1.0, 10.0).regime(), DampingRegime::Overdamped);
}

#[test]
fn params_reject_non_finite_non_positive_and_out_of_range() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0, 1e-4] {
        assert_eq!(
            SpringParams::new(bad, 1.0, 1.0),
            Err(SpringError::InvalidParams)
        );
        assert_eq!(
            SpringParams::new(1.0, bad, 1.0),
            Err(SpringError::InvalidParams)
        );
        assert_eq!(
            SpringParams::new(1.0, 1.0, bad),
            Err(SpringError::InvalidParams)
        );
    }
    assert!(SpringParams::new(MAX_MASS * 2.0, 1.0, 1.0).is_err());
    assert!(SpringParams::new(1.0, MAX_STIFFNESS * 2.0, 1.0).is_err());
    assert!(SpringParams::new(1.0, 1.0, MAX_DAMPING * 2.0).is_err());
    assert!(SpringParams::new(MIN_COEFFICIENT, MAX_STIFFNESS, MAX_DAMPING).is_ok());
}

#[test]
fn initial_state_is_reproduced_at_time_zero() {
    for profile in SpringProfile::ALL {
        let s = Spring::from_velocity(profile.params(), 3.0, -2.0, 7.5).unwrap();
        let st = s.sample(0.0).unwrap();
        assert_eq!(st.position, -2.0);
        assert_eq!(st.velocity, 7.5);
    }
}

/// `m·x'' + c·x' + k·(x − eq) = 0` using the analytic velocity and a central
/// difference of it for the acceleration.
#[test]
fn solution_satisfies_the_differential_equation_in_all_regimes() {
    let cases = [
        params(1.0, 100.0, 2.0),        // underdamped
        params(1.0, 100.0, 20.0),       // critical
        params(1.0, 100.0, 60.0),       // overdamped
        params(1.0, 100.0, 20.0000001), // just over critical
        params(2.0, 50.0, 0.01),        // nearly undamped
    ];
    let h = 1e-6;
    for p in cases {
        let s = Spring::from_velocity(p, 1.0, 4.0, -3.0)
            .unwrap()
            .with_constant_force(2.0)
            .unwrap();
        for t in [0.01, 0.1, 0.37, 1.0] {
            let st = s.sample(t).unwrap();
            let a =
                (s.sample(t + h).unwrap().velocity - s.sample(t - h).unwrap().velocity) / (2.0 * h);
            let dx =
                (s.sample(t + h).unwrap().position - s.sample(t - h).unwrap().position) / (2.0 * h);
            let residual = p.mass() * a
                + p.damping() * st.velocity
                + p.stiffness() * (st.position - s.equilibrium());
            assert!(residual.abs() < 1e-3, "{p:?} t={t} residual={residual}");
            assert!(close(dx, st.velocity, 1e-6), "{p:?} t={t}");
        }
    }
}

#[test]
fn motion_is_continuous_across_critical_damping() {
    let (m, k) = (1.0_f64, 400.0_f64);
    let critical = 2.0 * (k * m).sqrt();
    let reference = spring(params(m, k, critical), 1.0, 3.0);
    for delta in [1e-15, 1e-12, 1e-9, 1e-6, 1e-4] {
        for sign in [-1.0, 1.0] {
            let s = spring(params(m, k, critical * (1.0 + sign * delta)), 1.0, 3.0);
            for i in 0..=400 {
                let t = i as f64 * 0.0025;
                let a = s.sample(t).unwrap();
                let b = reference.sample(t).unwrap();
                assert!(a.position.is_finite() && a.velocity.is_finite());
                // Sensitivity to ζ is O(1), so differences scale with δ.
                assert!(
                    (a.position - b.position).abs() <= 10.0 * delta + 1e-13,
                    "δ={delta} t={t} {a:?} {b:?}"
                );
                assert!((a.velocity - b.velocity).abs() <= 500.0 * delta + 1e-11);
            }
        }
    }
    // Fine sweep of c through ζ = 1: neighbouring damping values never jump.
    let mut previous: Option<SpringState> = None;
    for i in -2000..=2000 {
        let c = critical * (1.0 + i as f64 * 1e-7);
        let st = spring(params(m, k, c), 1.0, 3.0).sample(0.2).unwrap();
        if let Some(prev) = previous {
            assert!((st.position - prev.position).abs() < 1e-7, "c={c}");
            assert!((st.velocity - prev.velocity).abs() < 1e-5, "c={c}");
        }
        previous = Some(st);
    }
}

#[test]
fn critical_damping_overshoots_with_strong_initial_velocity() {
    // Snappy: x(t) = (1 + (v0 + 20) t)·e^{-20t}; v0 = -40 crosses at t = 0.05.
    let s = spring(SpringProfile::Snappy.params(), 1.0, -40.0);
    let crossing = s.sample(0.05).unwrap().position;
    assert!(crossing.abs() < 1e-12);
    let min = (1..200)
        .map(|i| s.sample(i as f64 * 0.005).unwrap().position)
        .fold(f64::INFINITY, f64::min);
    assert!(min < -0.1, "expected overshoot, min={min}");
    // Without the push the same spring approaches monotonically.
    let calm = spring(SpringProfile::Snappy.params(), 1.0, 0.0);
    assert!((1..200).all(|i| calm.sample(i as f64 * 0.005).unwrap().position > 0.0));
}

#[test]
fn constant_force_shifts_equilibrium_analytically() {
    let p = SpringProfile::Gentle.params();
    let gravity = -9.81 * p.mass();
    let s = Spring::from_velocity(p, 10.0, 10.0, 0.0)
        .unwrap()
        .with_constant_force(gravity)
        .unwrap();
    let eq = 10.0 + gravity / p.stiffness();
    assert_eq!(s.equilibrium(), eq);
    let late = s.sample(60.0).unwrap();
    assert!((late.position - eq).abs() < 1e-12);
    assert!(late.velocity.abs() < 1e-12);
    assert!(s.sample(0.3).unwrap().position < 10.0);
    let base = Spring::from_velocity(p, 0.0, 0.0, 0.0).unwrap();
    for bad in [
        f64::NAN,
        f64::INFINITY,
        1e10,
        MAX_POSITION * p.stiffness() * 2.0,
    ] {
        assert_eq!(
            base.with_constant_force(bad),
            Err(SpringError::InvalidForce)
        );
    }
}

#[test]
fn sampling_is_independent_of_query_order() {
    let s = Spring::from_velocity(SpringProfile::Wobbly.params(), 5.0, -3.0, 12.0)
        .unwrap()
        .with_constant_force(1.5)
        .unwrap();
    let times: Vec<f64> = (0..240).map(|i| i as f64 / 24.0).collect();
    let forward: Vec<_> = times.iter().map(|&t| s.sample(t).unwrap()).collect();
    let mut reverse: Vec<_> = times.iter().rev().map(|&t| s.sample(t).unwrap()).collect();
    reverse.reverse();
    let bits = |v: &[SpringState]| {
        v.iter()
            .map(|s| (s.position.to_bits(), s.velocity.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&forward), bits(&reverse));
    // A scrambled order (stride 7 over 240) also matches.
    for i in 0..240 {
        let j = (i * 7) % 240;
        assert_eq!(s.sample(times[j]).unwrap(), forward[j]);
    }
}

#[test]
fn seeking_matches_restarting_from_an_intermediate_state() {
    for profile in SpringProfile::ALL {
        let s = Spring::from_velocity(profile.params(), 2.0, -1.0, 4.0).unwrap();
        for (t1, t2) in [(0.1, 0.25), (0.5, 0.5), (1.0, 0.01)] {
            let mid = s.sample(t1).unwrap();
            let restarted = Spring::new(profile.params(), 2.0, mid).unwrap();
            let a = restarted.sample(t2).unwrap();
            let b = s.sample(t1 + t2).unwrap();
            assert!(close(a.position, b.position, 1e-12), "{profile:?}");
            assert!(close(a.velocity, b.velocity, 1e-10), "{profile:?}");
        }
    }
}

#[test]
fn results_stay_finite_over_the_parameter_range() {
    let ms = [MIN_COEFFICIENT, 1.0, MAX_MASS];
    let ks = [MIN_COEFFICIENT, 1.0, MAX_STIFFNESS];
    let cs = [MIN_COEFFICIENT, 1.0, MAX_DAMPING];
    let states = [
        (MAX_POSITION, MAX_VELOCITY),
        (-MAX_POSITION, MAX_VELOCITY),
        (0.0, -MAX_VELOCITY),
        (1.0, 0.0),
    ];
    let times = [0.0, 1e-12, 1e-6, 1e-3, 1.0, 1e3, MAX_SPRING_TIME];
    for &m in &ms {
        for &k in &ks {
            for &c in &cs {
                let p = params(m, k, c);
                for &(x0, v0) in &states {
                    let s = Spring::from_velocity(p, -MAX_POSITION, x0, v0).unwrap();
                    for &t in &times {
                        let st = s.sample(t).unwrap();
                        assert!(
                            st.position.is_finite() && st.velocity.is_finite(),
                            "m={m} k={k} c={c} x0={x0} v0={v0} t={t}: {st:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rejects_invalid_times_and_states() {
    let s = spring(SpringProfile::Bouncy.params(), 1.0, 0.0);
    for bad in [f64::NAN, f64::INFINITY, -1e-9, MAX_SPRING_TIME * 1.5] {
        assert_eq!(s.sample(bad), Err(SpringError::InvalidTime));
    }
    let p = SpringProfile::Bouncy.params();
    assert_eq!(
        Spring::from_velocity(p, 0.0, f64::NAN, 0.0),
        Err(SpringError::InvalidPosition)
    );
    assert_eq!(
        Spring::from_velocity(p, MAX_POSITION * 2.0, 0.0, 0.0),
        Err(SpringError::InvalidPosition)
    );
    assert_eq!(
        Spring::from_velocity(p, 0.0, 0.0, f64::INFINITY),
        Err(SpringError::InvalidVelocity)
    );
    // Unvalidated struct literals are checked by the constructor too.
    let raw = SpringState {
        position: 0.0,
        velocity: MAX_VELOCITY * 2.0,
    };
    assert_eq!(Spring::new(p, 0.0, raw), Err(SpringError::InvalidVelocity));
}

fn linear_drag(start: f64, velocity: f64, n: usize, dt: f64) -> Vec<ReleaseSample> {
    (0..n)
        .map(|i| ReleaseSample {
            time: 2.0 + i as f64 * dt,
            position: start + velocity * i as f64 * dt,
        })
        .collect()
}

#[test]
fn release_uses_last_position_and_windowed_velocity() {
    let drag = linear_drag(10.0, 120.0, 12, 1.0 / 60.0);
    let st = release_state(&drag, 0.1).unwrap();
    assert_eq!(st.position, drag.last().unwrap().position);
    assert!((st.velocity - 120.0).abs() < 1e-9, "{st:?}");

    // Older samples with a different velocity fall outside the window.
    let mut mixed = linear_drag(0.0, -500.0, 10, 1.0 / 60.0);
    let t0 = mixed.last().unwrap().time;
    let x0 = mixed.last().unwrap().position;
    mixed.extend((1..=6).map(|i| ReleaseSample {
        time: t0 + i as f64 * 0.02,
        position: x0 + 40.0 * i as f64 * 0.02,
    }));
    let st = release_state(&mixed, 0.1).unwrap();
    assert!((st.velocity - 40.0).abs() < 1e-9, "{st:?}");

    let single = release_state(&drag[..1], 0.1).unwrap();
    assert_eq!(single, SpringState::new(10.0, 0.0).unwrap());
}

#[test]
fn release_has_no_jerk_at_the_handover() {
    let drag = linear_drag(10.0, 120.0, 12, 1.0 / 60.0);
    for profile in SpringProfile::ALL {
        let s = Spring::from_release(profile.params(), 0.0, &drag, 0.1).unwrap();
        let at = s.sample(0.0).unwrap();
        assert_eq!(at.position, drag.last().unwrap().position);
        assert!((at.velocity - 120.0).abs() < 1e-9);
        // Position continues along the drag direction to first order.
        let h = 1e-5;
        let next = s.sample(h).unwrap().position;
        assert!(
            ((next - at.position) / h - 120.0).abs() < 0.1,
            "{profile:?}"
        );
    }
}

#[test]
fn release_rejects_invalid_input() {
    let ok = linear_drag(0.0, 1.0, 3, 0.01);
    assert_eq!(release_state(&[], 0.1), Err(SpringError::InvalidRelease));
    for w in [0.0, -0.1, f64::NAN, 2.0] {
        assert_eq!(release_state(&ok, w), Err(SpringError::InvalidRelease));
    }
    let mut unordered = ok.clone();
    unordered.swap(0, 1);
    assert_eq!(
        release_state(&unordered, 0.1),
        Err(SpringError::InvalidRelease)
    );
    let mut nan = ok.clone();
    nan[1].time = f64::NAN;
    assert_eq!(release_state(&nan, 0.1), Err(SpringError::InvalidRelease));
    let mut far = ok.clone();
    far[2].position = f64::INFINITY;
    assert_eq!(release_state(&far, 0.1), Err(SpringError::InvalidPosition));
    let too_fast = [
        ReleaseSample {
            time: 0.0,
            position: 0.0,
        },
        ReleaseSample {
            time: 1e-12,
            position: 1000.0,
        },
    ];
    assert_eq!(
        release_state(&too_fast, 0.1),
        Err(SpringError::InvalidVelocity)
    );
    let long = linear_drag(0.0, 1.0, 1025, 1e-4);
    assert_eq!(release_state(&long, 0.1), Err(SpringError::InvalidRelease));
}

fn windy() -> ForceProfile {
    ForceProfile {
        constant: -2.0,
        gust: Some(Gust {
            amplitude: 3.0,
            frequency_hz: 0.7,
            phase: 0.3,
        }),
        turbulence: Some(Turbulence {
            amplitude: 1.5,
            cell_seconds: 0.25,
            seed: 42,
        }),
    }
}

#[test]
fn forced_spring_checkpoints_match_stepping_from_zero_bit_for_bit() {
    let s = ForcedSpring::new(
        SpringProfile::Bouncy.params(),
        0.0,
        SpringState::new(1.0, 0.0).unwrap(),
        windy(),
        5.0,
    )
    .unwrap();
    for t in [0.0, 0.0004, 0.5, 1.0, 1.0001, 2.345, 4.99, 5.0] {
        let a = s.sample(t).unwrap();
        let b = s.sample_from_start(t).unwrap();
        assert_eq!(a.position.to_bits(), b.position.to_bits(), "t={t}");
        assert_eq!(a.velocity.to_bits(), b.velocity.to_bits(), "t={t}");
    }
    assert_eq!(s.sample(5.01), Err(SpringError::BeyondHorizon(5.01)));
}

#[test]
fn forced_spring_is_frame_rate_and_order_independent() {
    let s = ForcedSpring::new(
        SpringProfile::Wobbly.params(),
        1.0,
        SpringState::default(),
        windy(),
        4.0,
    )
    .unwrap();
    // 24 fps and 60 fps share every whole second; values must be identical.
    let at24: Vec<_> = (0..=96)
        .map(|i| s.sample(i as f64 / 24.0).unwrap())
        .collect();
    let at60: Vec<_> = (0..=240)
        .map(|i| s.sample(i as f64 / 60.0).unwrap())
        .collect();
    for sec in 0..=4 {
        assert_eq!(at24[sec * 24], at60[sec * 60]);
    }
    let backwards: Vec<_> = (0..=96)
        .rev()
        .map(|i| s.sample(i as f64 / 24.0).unwrap())
        .collect();
    assert!(at24.iter().eq(backwards.iter().rev()));
    // Motion stays continuous between frames.
    for w in at60.windows(2) {
        assert!((w[1].position - w[0].position).abs() < 1.0);
    }
}

#[test]
fn forced_spring_with_constant_force_matches_the_analytic_solution() {
    let p = SpringProfile::Gentle.params();
    let init = SpringState::new(2.0, -1.0).unwrap();
    let forced = ForcedSpring::new(
        p,
        0.5,
        init,
        ForceProfile {
            constant: -9.81 * p.mass(),
            ..Default::default()
        },
        3.0,
    )
    .unwrap();
    let analytic = Spring::new(p, 0.5, init)
        .unwrap()
        .with_constant_force(-9.81 * p.mass())
        .unwrap();
    for i in 0..=30 {
        let t = i as f64 * 0.1;
        let a = forced.sample(t).unwrap();
        let b = analytic.sample(t).unwrap();
        assert!((a.position - b.position).abs() < 1e-9, "t={t}");
        assert!((a.velocity - b.velocity).abs() < 1e-8, "t={t}");
    }
}

#[test]
fn turbulence_is_seeded_bounded_and_stiff_springs_stay_stable() {
    let mut f = windy();
    let a: Vec<_> = (0..100).map(|i| f.evaluate(i as f64 * 0.037)).collect();
    let b: Vec<_> = (0..100).map(|i| f.evaluate(i as f64 * 0.037)).collect();
    assert_eq!(a, b);
    assert!(a.iter().all(|v| v.abs() <= 2.0 + 3.0 + 1.5));
    f.turbulence.as_mut().unwrap().seed = 43;
    let c: Vec<_> = (0..100).map(|i| f.evaluate(i as f64 * 0.037)).collect();
    assert_ne!(a, c);

    // Stiffest, lightest spring: ω_n ≈ 31623 rad/s, far above the step rate.
    let p = params(MIN_COEFFICIENT, MAX_STIFFNESS, MIN_COEFFICIENT);
    let s = ForcedSpring::new(p, 0.0, SpringState::new(1.0, 0.0).unwrap(), windy(), 2.0).unwrap();
    for i in 0..=200 {
        let st = s.sample(i as f64 * 0.01).unwrap();
        assert!(st.position.is_finite() && st.velocity.is_finite());
        assert!(st.position.abs() <= 1.0 + 1e-5, "{st:?}");
    }
}

#[test]
fn forced_spring_rejects_invalid_configuration() {
    let p = SpringProfile::Snappy.params();
    let init = SpringState::default();
    for h in [f64::NAN, -1.0, MAX_FORCED_HORIZON + 1.0] {
        assert_eq!(
            ForcedSpring::new(p, 0.0, init, ForceProfile::default(), h),
            Err(SpringError::InvalidHorizon)
        );
    }
    let bad_forces = [
        ForceProfile {
            constant: f64::NAN,
            ..Default::default()
        },
        ForceProfile {
            gust: Some(Gust {
                amplitude: 1.0,
                frequency_hz: 1000.0,
                phase: 0.0,
            }),
            ..Default::default()
        },
        ForceProfile {
            turbulence: Some(Turbulence {
                amplitude: 1.0,
                cell_seconds: 0.0,
                seed: 0,
            }),
            ..Default::default()
        },
        // Peak force would push the equilibrium out of bounds.
        ForceProfile {
            constant: MAX_POSITION * 400.0,
            gust: Some(Gust {
                amplitude: MAX_POSITION * 400.0,
                frequency_hz: 1.0,
                phase: 0.0,
            }),
            ..Default::default()
        },
    ];
    for f in bad_forces {
        assert_eq!(
            ForcedSpring::new(p, 0.0, init, f, 1.0),
            Err(SpringError::InvalidForce),
            "{f:?}"
        );
    }
}
