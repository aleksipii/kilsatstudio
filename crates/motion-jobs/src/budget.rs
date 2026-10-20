//! Process-wide byte accounting for large allocations (decoded images, audio buffers,
//! render caches). This is *accounting*, not a measured RSS limit or an allocator:
//! callers reserve an estimate before allocating and the RAII `Reservation` releases it
//! on drop. Refusal is deterministic: the same sequence of reservations always succeeds
//! or fails identically, independent of the actual system memory state.
use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

/// Default capacity of [`process_budget`]: 1 GiB of estimated bulk allocations.
pub const DEFAULT_PROCESS_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetExhausted {
    pub label: String,
    pub requested: u64,
    pub available: u64,
    pub capacity: u64,
}
impl fmt::Display for BudgetExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "memory budget exhausted: {} needs {} bytes but only {} of {} bytes are available; \
             close or clear other large assets (images, audio, caches) and retry",
            self.label, self.requested, self.available, self.capacity
        )
    }
}
impl std::error::Error for BudgetExhausted {}

struct Inner {
    capacity: u64,
    used: AtomicU64,
}

/// Cloneable handle to one shared byte budget.
#[derive(Clone)]
pub struct MemoryBudget(Arc<Inner>);

impl fmt::Debug for MemoryBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MemoryBudget({}/{})", self.used(), self.capacity())
    }
}

impl MemoryBudget {
    pub fn new(capacity: u64) -> Self {
        Self(Arc::new(Inner {
            capacity,
            used: AtomicU64::new(0),
        }))
    }
    pub fn capacity(&self) -> u64 {
        self.0.capacity
    }
    pub fn used(&self) -> u64 {
        self.0.used.load(Ordering::Acquire)
    }
    pub fn available(&self) -> u64 {
        self.0.capacity.saturating_sub(self.used())
    }
    /// Atomically reserve `bytes`, or refuse without changing the budget.
    pub fn reserve(&self, label: &str, bytes: u64) -> Result<Reservation, BudgetExhausted> {
        let mut used = self.0.used.load(Ordering::Acquire);
        loop {
            let next = used
                .checked_add(bytes)
                .filter(|n| *n <= self.0.capacity)
                .ok_or_else(|| BudgetExhausted {
                    label: label.to_string(),
                    requested: bytes,
                    available: self.0.capacity.saturating_sub(used),
                    capacity: self.0.capacity,
                })?;
            match self
                .0
                .used
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    return Ok(Reservation {
                        budget: self.clone(),
                        bytes,
                    })
                }
                Err(actual) => used = actual,
            }
        }
    }
    /// Reserve the estimated bytes of `frames` x `channels` interleaved `f32` PCM.
    pub fn reserve_pcm(
        &self,
        label: &str,
        frames: u64,
        channels: u16,
    ) -> Result<Reservation, BudgetExhausted> {
        self.reserve(label, pcm_bytes(frames, channels))
    }
    /// Reserve the estimated bytes of one RGBA8 frame.
    pub fn reserve_rgba(
        &self,
        label: &str,
        width: u32,
        height: u32,
    ) -> Result<Reservation, BudgetExhausted> {
        self.reserve(label, rgba_bytes(width, height))
    }
}

/// Estimated in-memory size of interleaved `f32` PCM: `frames * channels * 4`.
/// Saturates at `u64::MAX` (which no budget can grant) instead of wrapping.
pub fn pcm_bytes(frames: u64, channels: u16) -> u64 {
    frames
        .saturating_mul(u64::from(channels))
        .saturating_mul(std::mem::size_of::<f32>() as u64)
}

/// Estimated size of a straight RGBA8 frame: `width * height * 4`. Saturating.
pub fn rgba_bytes(width: u32, height: u32) -> u64 {
    u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4)
}

/// Bytes held against a budget; released exactly once on drop. Share with `Arc`.
#[derive(Debug)]
pub struct Reservation {
    budget: MemoryBudget,
    bytes: u64,
}
impl Reservation {
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.0.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

/// The single budget shared by all features of the application process.
pub fn process_budget() -> &'static MemoryBudget {
    static BUDGET: OnceLock<MemoryBudget> = OnceLock::new();
    BUDGET.get_or_init(|| MemoryBudget::new(DEFAULT_PROCESS_BYTES))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustion_is_deterministic_and_release_restores_capacity() {
        let budget = MemoryBudget::new(100);
        let a = budget.reserve("a", 60).unwrap();
        let err = budget.reserve("b", 41).unwrap_err();
        assert_eq!((err.requested, err.available, err.capacity), (41, 40, 100));
        assert!(err.to_string().contains("b needs 41 bytes"));
        assert_eq!(budget.used(), 60);
        let b = budget.reserve("b", 40).unwrap();
        assert_eq!(budget.available(), 0);
        drop(a);
        assert_eq!(budget.used(), 40);
        drop(b);
        assert_eq!(budget.used(), 0);
        assert!(budget.reserve("overflow", u64::MAX).is_err());
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.reserve("zero", 0).unwrap().bytes(), 0);
    }
    #[test]
    fn concurrent_reservations_never_exceed_capacity() {
        let budget = MemoryBudget::new(1000);
        let peak = AtomicU64::new(0);
        let granted = AtomicU64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..2000 {
                        if let Ok(r) = budget.reserve("t", 100) {
                            peak.fetch_max(budget.used(), Ordering::SeqCst);
                            granted.fetch_add(1, Ordering::SeqCst);
                            drop(r);
                        }
                    }
                });
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= 1000);
        assert!(granted.load(Ordering::SeqCst) > 0);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn concurrent_held_reservations_grant_exactly_capacity() {
        let budget = MemoryBudget::new(1000);
        let held = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..50 {
                        if let Ok(r) = budget.reserve("t", 10) {
                            held.lock().unwrap().push(r);
                        }
                    }
                });
            }
        });
        assert_eq!(held.lock().unwrap().len(), 100);
        assert_eq!(budget.used(), 1000);
    }
    #[test]
    fn pcm_and_rgba_estimates_are_exact_and_saturate() {
        assert_eq!(pcm_bytes(24_000, 2), 192_000);
        assert_eq!(rgba_bytes(1280, 720), 3_686_400);
        assert_eq!(pcm_bytes(u64::MAX, 2), u64::MAX);
        let budget = MemoryBudget::new(1 << 20);
        assert!(budget.reserve_pcm("huge", u64::MAX, 2).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn typed_reservations_release_on_drop_and_report_actionable_errors() {
        let budget = MemoryBudget::new(1000);
        let pcm = budget.reserve_pcm("cue door.wav", 100, 1).unwrap(); // 400
        let frame = budget.reserve_rgba("preview frame", 10, 10).unwrap(); // 400
        assert_eq!(budget.used(), 800);
        let err = budget.reserve_pcm("cue sting.wav", 100, 1).unwrap_err();
        assert_eq!((err.requested, err.available), (400, 200));
        let text = err.to_string();
        assert!(text.contains("cue sting.wav") && text.contains("close or clear"));
        assert_eq!(budget.used(), 800, "a refused request changes nothing");
        drop(pcm);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn reservations_are_released_when_a_failing_scope_unwinds_with_error() {
        let budget = MemoryBudget::new(1000);
        let run = || -> Result<(), BudgetExhausted> {
            let _a = budget.reserve("a", 600)?;
            let _b = budget.reserve("b", 600)?; // fails; `_a` must still be released
            Ok(())
        };
        assert!(run().is_err());
        assert_eq!(budget.used(), 0);
        let panicked = std::panic::catch_unwind(|| {
            let _a = budget.reserve("a", 100).unwrap();
            panic!("boom");
        });
        assert!(panicked.is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn process_budget_is_shared() {
        assert_eq!(process_budget().capacity(), DEFAULT_PROCESS_BYTES);
        assert!(std::ptr::eq(process_budget(), process_budget()));
    }
}
