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
    fn process_budget_is_shared() {
        assert_eq!(process_budget().capacity(), DEFAULT_PROCESS_BYTES);
        assert!(std::ptr::eq(process_budget(), process_budget()));
    }
}
