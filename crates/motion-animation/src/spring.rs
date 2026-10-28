//! Damped springs evaluated as pure functions of local time.
//!
//! The model is `m·x'' + c·x' + k·(x − target) = F(t)` in one dimension.
//! Without a time-dependent force the state is the closed-form solution, so
//! sampling never depends on frame rate or on the order of queries. A
//! time-dependent force (wind, turbulence) is integrated on a fixed step that is
//! independent of frame rate; every query starts from `t = 0` or from a
//! checkpoint produced by the same step sequence, so results are bit-identical
//! regardless of query order. See `docs/physics-springs.md`.

/// Smallest accepted mass, stiffness or damping coefficient.
pub const MIN_COEFFICIENT: f64 = 1e-3;
/// Largest accepted mass.
pub const MAX_MASS: f64 = 1e3;
/// Largest accepted stiffness.
pub const MAX_STIFFNESS: f64 = 1e6;
/// Largest accepted damping coefficient.
pub const MAX_DAMPING: f64 = 1e6;
/// Largest accepted absolute position, target or constant-force equilibrium.
pub const MAX_POSITION: f64 = 1e6;
/// Largest accepted absolute velocity.
pub const MAX_VELOCITY: f64 = 1e8;
/// Largest accepted absolute force.
pub const MAX_FORCE: f64 = 1e9;
/// Largest accepted local spring time in seconds.
pub const MAX_SPRING_TIME: f64 = 1e6;
/// Fixed integration step for time-dependent forces (1/1200 s).
pub const FORCED_STEP_SECONDS: f64 = 1.0 / 1200.0;
/// Steps between stored checkpoints of a forced spring (one second).
pub const FORCED_CHECKPOINT_STEPS: usize = 1200;
/// Longest precomputed horizon of a forced spring in seconds.
pub const MAX_FORCED_HORIZON: f64 = 3600.0;
/// Largest number of drag samples accepted for a release estimate.
pub const MAX_RELEASE_SAMPLES: usize = 1024;
/// Largest rolling window for a release velocity estimate in seconds.
pub const MAX_RELEASE_WINDOW: f64 = 1.0;

/// Below this value of `|s|·t²` the propagator uses its Taylor series, which
/// makes the solution continuous (and exactly critical at `s = 0`) across ζ = 1.
const SERIES_THRESHOLD: f64 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SpringError {
    #[error(
        "spring mass must be in {MIN_COEFFICIENT}..={MAX_MASS}, stiffness in \
         {MIN_COEFFICIENT}..={MAX_STIFFNESS} and damping in {MIN_COEFFICIENT}..={MAX_DAMPING}"
    )]
    InvalidParams,
    #[error("spring position and target must be finite with |value| <= {MAX_POSITION}")]
    InvalidPosition,
    #[error("spring velocity must be finite with |value| <= {MAX_VELOCITY}")]
    InvalidVelocity,
    #[error(
        "spring force must be finite with |force| <= {MAX_FORCE} and keep the equilibrium \
         within {MAX_POSITION}"
    )]
    InvalidForce,
    #[error("spring time must be finite and in 0..={MAX_SPRING_TIME} seconds")]
    InvalidTime,
    #[error("forced spring horizon must be finite and in 0..={MAX_FORCED_HORIZON} seconds")]
    InvalidHorizon,
    #[error("time {0} s is beyond the forced spring horizon")]
    BeyondHorizon(f64),
    #[error(
        "release needs 1..={MAX_RELEASE_SAMPLES} samples with finite strictly increasing \
         times and a window in (0, {MAX_RELEASE_WINDOW}] seconds"
    )]
    InvalidRelease,
}

/// Validated physical coefficients of a damped spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams {
    mass: f64,
    stiffness: f64,
    damping: f64,
}

impl SpringParams {
    pub fn new(mass: f64, stiffness: f64, damping: f64) -> Result<Self, SpringError> {
        let ok = |v: f64, max: f64| v.is_finite() && (MIN_COEFFICIENT..=max).contains(&v);
        if !ok(mass, MAX_MASS) || !ok(stiffness, MAX_STIFFNESS) || !ok(damping, MAX_DAMPING) {
            return Err(SpringError::InvalidParams);
        }
        Ok(Self {
            mass,
            stiffness,
            damping,
        })
    }
    pub fn mass(&self) -> f64 {
        self.mass
    }
    pub fn stiffness(&self) -> f64 {
        self.stiffness
    }
    pub fn damping(&self) -> f64 {
        self.damping
    }
    /// Undamped angular frequency `ω_n = √(k/m)` in radians per second.
    pub fn natural_frequency(&self) -> f64 {
        (self.stiffness / self.mass).sqrt()
    }
    /// Dimensionless damping ratio `ζ = c / (2√(k·m))`.
    pub fn damping_ratio(&self) -> f64 {
        self.damping / (2.0 * (self.stiffness * self.mass).sqrt())
    }
    /// Qualitative regime; `Critical` only when `c² = 4km` exactly in `f64`.
    pub fn regime(&self) -> DampingRegime {
        let s = self.discriminant();
        if s > 0.0 {
            DampingRegime::Underdamped
        } else if s < 0.0 {
            DampingRegime::Overdamped
        } else {
            DampingRegime::Critical
        }
    }
    /// Decay rate `a = c / 2m`.
    fn decay(&self) -> f64 {
        self.damping / (2.0 * self.mass)
    }
    /// `s = ω_n² − a² = (4km − c²) / 4m²`; positive when underdamped.
    fn discriminant(&self) -> f64 {
        (4.0 * self.stiffness * self.mass - self.damping * self.damping)
            / (4.0 * self.mass * self.mass)
    }
    fn omega_sq(&self) -> f64 {
        self.stiffness / self.mass
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DampingRegime {
    Underdamped,
    Critical,
    Overdamped,
}

/// Named material profiles from the architecture report (section 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpringProfile {
    /// m 1.0, k 400, c 40: ζ = 1.00.
    Snappy,
    /// m 1.5, k 100, c 15: ζ ≈ 0.61.
    Gentle,
    /// m 0.8, k 250, c 8: ζ ≈ 0.28.
    Bouncy,
    /// m 2.0, k 800, c 80: ζ = 1.00.
    Stiff,
    /// m 0.5, k 120, c 3: ζ ≈ 0.19.
    Wobbly,
}

impl SpringProfile {
    pub const ALL: [SpringProfile; 5] = [
        SpringProfile::Snappy,
        SpringProfile::Gentle,
        SpringProfile::Bouncy,
        SpringProfile::Stiff,
        SpringProfile::Wobbly,
    ];
    /// `(mass, stiffness, damping)` exactly as tabulated.
    pub const fn coefficients(self) -> (f64, f64, f64) {
        match self {
            SpringProfile::Snappy => (1.0, 400.0, 40.0),
            SpringProfile::Gentle => (1.5, 100.0, 15.0),
            SpringProfile::Bouncy => (0.8, 250.0, 8.0),
            SpringProfile::Stiff => (2.0, 800.0, 80.0),
            SpringProfile::Wobbly => (0.5, 120.0, 3.0),
        }
    }
    pub fn params(self) -> SpringParams {
        let (m, k, c) = self.coefficients();
        SpringParams {
            mass: m,
            stiffness: k,
            damping: c,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            SpringProfile::Snappy => "snappy",
            SpringProfile::Gentle => "gentle",
            SpringProfile::Bouncy => "bouncy",
            SpringProfile::Stiff => "stiff",
            SpringProfile::Wobbly => "wobbly",
        }
    }
}

/// Position and velocity of a spring at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpringState {
    pub position: f64,
    pub velocity: f64,
}

impl SpringState {
    pub fn new(position: f64, velocity: f64) -> Result<Self, SpringError> {
        check_position(position)?;
        if !velocity.is_finite() || velocity.abs() > MAX_VELOCITY {
            return Err(SpringError::InvalidVelocity);
        }
        Ok(Self { position, velocity })
    }
}

fn check_position(value: f64) -> Result<(), SpringError> {
    if value.is_finite() && value.abs() <= MAX_POSITION {
        Ok(())
    } else {
        Err(SpringError::InvalidPosition)
    }
}

fn check_time(time: f64) -> Result<(), SpringError> {
    if time.is_finite() && (0.0..=MAX_SPRING_TIME).contains(&time) {
        Ok(())
    } else {
        Err(SpringError::InvalidTime)
    }
}

fn equilibrium(params: &SpringParams, target: f64, force: f64) -> Result<f64, SpringError> {
    if !force.is_finite() || force.abs() > MAX_FORCE {
        return Err(SpringError::InvalidForce);
    }
    let eq = target + force / params.stiffness;
    if !eq.is_finite() || eq.abs() > MAX_POSITION {
        return Err(SpringError::InvalidForce);
    }
    Ok(eq)
}

/// Unforced propagator `(x, v) ↦ (x(t), v(t))` for a displacement from
/// equilibrium. With `y = e^{a t}·x` the equation becomes `y'' = −s·y`, whose
/// solution is `y = x0·C + (v0 + a·x0)·S` with `C = cos(√s t)`,
/// `S = sin(√s t)/√s` (hyperbolic when `s < 0`, `C = 1`, `S = t` when `s = 0`).
/// Returned values already include `e^{−a t}`:
/// `x = x0·EC + (v0 + a x0)·ES`, `v = v0·EC − (ω² x0 + a v0)·ES`.
fn propagate(params: &SpringParams, x0: f64, v0: f64, t: f64) -> (f64, f64) {
    let a = params.decay();
    let w2 = params.omega_sq();
    let s = params.discriminant();
    let st2 = s * t * t;
    let (ec, es) = if st2.abs() < SERIES_THRESHOLD {
        // Taylor series of cos/sinc in s·t²; exact critical case at s = 0.
        let e = (-a * t).exp();
        let c = 1.0 - st2 / 2.0 + st2 * st2 / 24.0;
        let sn = t * (1.0 - st2 / 6.0 + st2 * st2 / 120.0);
        (e * c, e * sn)
    } else if s > 0.0 {
        let wd = s.sqrt();
        let e = (-a * t).exp();
        (e * (wd * t).cos(), e * (wd * t).sin() / wd)
    } else {
        // Overdamped: combine exponents so e^{−at}·cosh(βt) cannot form ∞·0.
        let beta = (-s).sqrt();
        let r1 = -w2 / (a + beta); // = β − a, computed without cancellation
        let r2 = -(a + beta);
        let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
        ((e1 + e2) / 2.0, (e1 - e2) / (2.0 * beta))
    };
    (
        x0 * ec + (v0 + a * x0) * es,
        v0 * ec - (w2 * x0 + a * v0) * es,
    )
}

/// A spring with optional constant external force, solved in closed form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    params: SpringParams,
    target: f64,
    equilibrium: f64,
    initial: SpringState,
}

impl Spring {
    /// Spring released at local time zero from `initial` towards `target`.
    pub fn new(
        params: SpringParams,
        target: f64,
        initial: SpringState,
    ) -> Result<Self, SpringError> {
        check_position(target)?;
        let initial = SpringState::new(initial.position, initial.velocity)?;
        Ok(Self {
            params,
            target,
            equilibrium: target,
            initial,
        })
    }

    /// Spring starting at `position` with `velocity` (e.g. a flick).
    pub fn from_velocity(
        params: SpringParams,
        target: f64,
        position: f64,
        velocity: f64,
    ) -> Result<Self, SpringError> {
        Self::new(params, target, SpringState::new(position, velocity)?)
    }

    /// Spring whose initial state is estimated from the end of a drag.
    pub fn from_release(
        params: SpringParams,
        target: f64,
        samples: &[ReleaseSample],
        window_seconds: f64,
    ) -> Result<Self, SpringError> {
        Self::new(params, target, release_state(samples, window_seconds)?)
    }

    /// Adds a constant force (e.g. gravity `m·g`); the rest position shifts to
    /// `target + F/k` and the motion stays analytic.
    pub fn with_constant_force(mut self, force: f64) -> Result<Self, SpringError> {
        self.equilibrium = equilibrium(&self.params, self.target, force)?;
        Ok(self)
    }

    pub fn params(&self) -> SpringParams {
        self.params
    }
    pub fn target(&self) -> f64 {
        self.target
    }
    /// Rest position including any constant force.
    pub fn equilibrium(&self) -> f64 {
        self.equilibrium
    }
    pub fn initial(&self) -> SpringState {
        self.initial
    }

    /// State at local time `time` seconds after release.
    pub fn sample(&self, time: f64) -> Result<SpringState, SpringError> {
        check_time(time)?;
        let (x, v) = propagate(
            &self.params,
            self.initial.position - self.equilibrium,
            self.initial.velocity,
            time,
        );
        Ok(SpringState {
            position: self.equilibrium + x,
            velocity: v,
        })
    }
}

/// Sinusoidal gust `amplitude · sin(2π·frequency·t + phase)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gust {
    pub amplitude: f64,
    pub frequency_hz: f64,
    pub phase: f64,
}

/// Seeded value noise in `[-amplitude, amplitude]`, smoothly interpolated
/// between lattice points every `cell_seconds`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turbulence {
    pub amplitude: f64,
    pub cell_seconds: f64,
    pub seed: u64,
}

/// Deterministic, time-dependent external force description.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ForceProfile {
    pub constant: f64,
    pub gust: Option<Gust>,
    pub turbulence: Option<Turbulence>,
}

impl ForceProfile {
    fn validate(&self) -> Result<(), SpringError> {
        let bounded = |v: f64| v.is_finite() && v.abs() <= MAX_FORCE;
        let mut peak = self.constant.abs();
        if !bounded(self.constant) {
            return Err(SpringError::InvalidForce);
        }
        if let Some(g) = self.gust {
            if !bounded(g.amplitude)
                || !g.frequency_hz.is_finite()
                || !(0.0..=100.0).contains(&g.frequency_hz)
                || !g.phase.is_finite()
                || g.phase.abs() > 1e3
            {
                return Err(SpringError::InvalidForce);
            }
            peak += g.amplitude.abs();
        }
        if let Some(n) = self.turbulence {
            if !bounded(n.amplitude)
                || !n.cell_seconds.is_finite()
                || !(1e-3..=1e3).contains(&n.cell_seconds)
            {
                return Err(SpringError::InvalidForce);
            }
            peak += n.amplitude.abs();
        }
        if peak > MAX_FORCE {
            return Err(SpringError::InvalidForce);
        }
        Ok(())
    }

    /// Force at time `t`; pure and deterministic.
    pub fn evaluate(&self, t: f64) -> f64 {
        let mut f = self.constant;
        if let Some(g) = self.gust {
            f += g.amplitude * (std::f64::consts::TAU * g.frequency_hz * t + g.phase).sin();
        }
        if let Some(n) = self.turbulence {
            let u = t / n.cell_seconds;
            let cell = u.floor();
            let frac = u - cell;
            let w = frac * frac * (3.0 - 2.0 * frac);
            let a = lattice(n.seed, cell as i64);
            let b = lattice(n.seed, cell as i64 + 1);
            f += n.amplitude * (a + (b - a) * w);
        }
        f
    }
}

/// Hash of a lattice index to `[-1, 1]` (SplitMix64 finaliser).
fn lattice(seed: u64, index: i64) -> f64 {
    let mut z = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// A spring driven by a time-dependent force. Each fixed step of
/// [`FORCED_STEP_SECONDS`] advances the exact unforced propagator around the
/// equilibrium `target + F(t_mid)/k`, holding the force at the step midpoint;
/// this is stable for every accepted stiffness. Checkpoints every second are
/// produced by the same step sequence, so a query reproduces exactly the
/// result of stepping from zero.
#[derive(Debug, Clone, PartialEq)]
pub struct ForcedSpring {
    params: SpringParams,
    target: f64,
    force: ForceProfile,
    horizon: f64,
    checkpoints: Vec<SpringState>,
}

impl ForcedSpring {
    pub fn new(
        params: SpringParams,
        target: f64,
        initial: SpringState,
        force: ForceProfile,
        horizon_seconds: f64,
    ) -> Result<Self, SpringError> {
        check_position(target)?;
        let initial = SpringState::new(initial.position, initial.velocity)?;
        force.validate()?;
        // Every equilibrium the force can produce must stay bounded.
        let peak = force.constant.abs()
            + force.gust.map_or(0.0, |g| g.amplitude.abs())
            + force.turbulence.map_or(0.0, |n| n.amplitude.abs());
        equilibrium(&params, target.abs(), peak)?;
        if !horizon_seconds.is_finite() || !(0.0..=MAX_FORCED_HORIZON).contains(&horizon_seconds) {
            return Err(SpringError::InvalidHorizon);
        }
        let mut spring = Self {
            params,
            target,
            force,
            horizon: horizon_seconds,
            checkpoints: vec![initial],
        };
        let total_steps = (horizon_seconds / FORCED_STEP_SECONDS).floor() as usize;
        let mut state = initial;
        let mut step = 0;
        while step + FORCED_CHECKPOINT_STEPS <= total_steps {
            for _ in 0..FORCED_CHECKPOINT_STEPS {
                state = spring.step(state, step, FORCED_STEP_SECONDS);
                step += 1;
            }
            spring.checkpoints.push(state);
        }
        Ok(spring)
    }

    pub fn horizon(&self) -> f64 {
        self.horizon
    }
    pub fn force(&self) -> ForceProfile {
        self.force
    }

    fn step(&self, state: SpringState, index: usize, dt: f64) -> SpringState {
        let start = index as f64 * FORCED_STEP_SECONDS;
        let mid = start + dt / 2.0;
        let eq = self.target + self.force.evaluate(mid) / self.params.stiffness;
        let (x, v) = propagate(&self.params, state.position - eq, state.velocity, dt);
        // Clamp to the accepted state bounds; with bounded forces this never
        // triggers in practice but guarantees finite output.
        SpringState {
            position: (eq + x).clamp(-2.0 * MAX_POSITION, 2.0 * MAX_POSITION),
            velocity: v.clamp(-MAX_VELOCITY, MAX_VELOCITY),
        }
    }

    /// State at local time `time` in `0..=horizon`.
    pub fn sample(&self, time: f64) -> Result<SpringState, SpringError> {
        check_time(time)?;
        if time > self.horizon {
            return Err(SpringError::BeyondHorizon(time));
        }
        let steps = (time / FORCED_STEP_SECONDS).floor() as usize;
        let checkpoint = (steps / FORCED_CHECKPOINT_STEPS).min(self.checkpoints.len() - 1);
        let mut state = self.checkpoints[checkpoint];
        for index in checkpoint * FORCED_CHECKPOINT_STEPS..steps {
            state = self.step(state, index, FORCED_STEP_SECONDS);
        }
        let rest = time - steps as f64 * FORCED_STEP_SECONDS;
        if rest > 0.0 {
            state = self.step(state, steps, rest);
        }
        Ok(state)
    }

    /// Reference evaluation stepping from zero without checkpoints.
    pub fn sample_from_start(&self, time: f64) -> Result<SpringState, SpringError> {
        check_time(time)?;
        if time > self.horizon {
            return Err(SpringError::BeyondHorizon(time));
        }
        let steps = (time / FORCED_STEP_SECONDS).floor() as usize;
        let mut state = self.checkpoints[0];
        for index in 0..steps {
            state = self.step(state, index, FORCED_STEP_SECONDS);
        }
        let rest = time - steps as f64 * FORCED_STEP_SECONDS;
        if rest > 0.0 {
            state = self.step(state, steps, rest);
        }
        Ok(state)
    }
}

/// One drag position sample, passed as plain values (no EventLog dependency).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseSample {
    pub time: f64,
    pub position: f64,
}

/// Initial spring state at the end of a drag: the last sample's position and
/// the least-squares velocity over samples within `window_seconds` before it.
/// A single sample in the window yields zero velocity.
pub fn release_state(
    samples: &[ReleaseSample],
    window_seconds: f64,
) -> Result<SpringState, SpringError> {
    if samples.is_empty()
        || samples.len() > MAX_RELEASE_SAMPLES
        || !window_seconds.is_finite()
        || !(window_seconds > 0.0 && window_seconds <= MAX_RELEASE_WINDOW)
        || samples.iter().any(|s| !s.time.is_finite())
        || samples.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err(SpringError::InvalidRelease);
    }
    for s in samples {
        check_position(s.position)?;
    }
    let last = samples[samples.len() - 1];
    let window: Vec<_> = samples
        .iter()
        .filter(|s| last.time - s.time <= window_seconds)
        .collect();
    let velocity = if window.len() < 2 {
        0.0
    } else {
        let n = window.len() as f64;
        // Centre on the last sample to keep sums well conditioned.
        let (mut st, mut sx) = (0.0, 0.0);
        for s in &window {
            st += s.time - last.time;
            sx += s.position - last.position;
        }
        let (mt, mx) = (st / n, sx / n);
        let (mut num, mut den) = (0.0, 0.0);
        for s in &window {
            let dt = s.time - last.time - mt;
            num += dt * (s.position - last.position - mx);
            den += dt * dt;
        }
        num / den
    };
    SpringState::new(last.position, velocity)
}

/// Largest accepted secondary-motion angle offset (half a turn), in radians.
pub const MAX_DANGLE_ANGLE: f64 = std::f64::consts::PI;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DangleError {
    #[error("dangle joint {0} is not in the rig")]
    UnknownJoint(String),
    #[error("dangle joint {0} is a hierarchy root; rotating it would move the whole character")]
    Root(String),
    #[error("dangle joint {joint} is the {role}; IK, grips and ground contact own it")]
    Reserved { joint: String, role: &'static str },
    #[error("dangle joint {joint} is an ancestor of {reserved}; its motion would move that limb")]
    MovesReserved { joint: String, reserved: String },
    #[error("dangle angle must be finite with |angle| <= {MAX_DANGLE_ANGLE} rad")]
    InvalidAngle,
}

/// Role of a joint in the A05 attachment frame, if it has one.
fn reserved_role(name: &str) -> Option<&'static str> {
    if name == motion_rig::PELVIS {
        return Some("pelvis");
    }
    if name == motion_rig::TORSO {
        return Some("arm anchor (torso)");
    }
    motion_rig::STICK_LIMBS.iter().find_map(|limb| {
        if name == limb.upper || name == limb.lower {
            Some("two-bone IK chain")
        } else if name == limb.end && limb.anchor == motion_rig::TORSO {
            Some("hand (IK end and grip)")
        } else if name == limb.end {
            Some("foot (IK end and ground contact)")
        } else {
            None
        }
    })
}

/// Index of a joint that may receive secondary (dangle) motion.
///
/// The joint must exist, must not be a root, must not be one of A05's
/// attachment-frame joints (pelvis, torso, the twelve limb joints of
/// [`motion_rig::STICK_LIMBS`]) and must not be an ancestor of any of them, so
/// a dangle never moves an IK chain, a held prop or a planted foot. On the
/// current stick characters this leaves the head and any extra joints such as
/// hair, ears, scarves or a keychain below a hand. See `docs/physics-springs.md`.
pub fn dangle_joint(rig: &motion_rig::PreparedRig, joint: &str) -> Result<usize, DangleError> {
    let index = rig
        .joint_index(joint)
        .ok_or_else(|| DangleError::UnknownJoint(joint.into()))?;
    let joints = rig.rig().joints();
    if joints[index].parent.is_none() {
        return Err(DangleError::Root(joint.into()));
    }
    if let Some(role) = reserved_role(joint) {
        return Err(DangleError::Reserved {
            joint: joint.into(),
            role,
        });
    }
    for reserved in joints.iter().filter(|j| reserved_role(&j.name).is_some()) {
        let mut parent = reserved.parent;
        while let Some(p) = parent {
            if p == index {
                return Err(DangleError::MovesReserved {
                    joint: joint.into(),
                    reserved: reserved.name.clone(),
                });
            }
            parent = joints[p].parent;
        }
    }
    Ok(index)
}

/// Adds a spring angle to a joint's animated offset. Only the rotation
/// changes; the offset's translation and scale are kept. The rig evaluator
/// clamps `rest + offset` to the joint's limits, so the spring state itself
/// does not know about limits.
pub fn apply_dangle(
    offset: motion_rig::Transform2,
    angle: f64,
) -> Result<motion_rig::Transform2, DangleError> {
    if !angle.is_finite() || angle.abs() > MAX_DANGLE_ANGLE {
        return Err(DangleError::InvalidAngle);
    }
    Ok(motion_rig::Transform2 {
        rotation: offset.rotation + angle,
        ..offset
    })
}
