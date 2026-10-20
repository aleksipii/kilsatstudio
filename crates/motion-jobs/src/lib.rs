//! Fixed scoped workers with bounded batches and ordered coordinator delivery.
use std::sync::{mpsc, Mutex};

mod budget;
mod graph;
pub use budget::{
    process_budget, BudgetExhausted, MemoryBudget, Reservation, DEFAULT_PROCESS_BYTES,
};
pub use graph::{Graph, GraphError, Outcome, Results, MAX_GRAPH_JOBS};

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub workers: usize,
    pub batch_size: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum JobError<E> {
    InvalidConfig,
    Cancelled,
    Task(E),
    Panic,
}

/// Runs at most `batch_size` outstanding jobs, using at most four workers.
/// `consume` runs on the caller in index order. On failure all workers are joined;
/// running tasks cannot be forcibly interrupted and should check cancellation too.
pub fn ordered_map<T: Send, E: Send>(
    total: usize,
    config: Config,
    cancelled: impl Fn() -> bool + Sync,
    map: impl Fn(usize) -> Result<T, E> + Sync,
    mut consume: impl FnMut(usize, T) -> Result<(), E>,
) -> Result<(), JobError<E>> {
    if !(1..=4).contains(&config.workers) || config.batch_size == 0 || config.batch_size > 16 {
        return Err(JobError::InvalidConfig);
    }
    let (send, receive) = mpsc::sync_channel(config.batch_size);
    let receive = Mutex::new(receive);
    let (results, output) = mpsc::sync_channel(config.batch_size);
    std::thread::scope(|scope| {
        for _ in 0..config.workers {
            let receive = &receive;
            let results = results.clone();
            let map = &map;
            let cancelled = &cancelled;
            scope.spawn(move || loop {
                let job = receive.lock().expect("job receiver poisoned").recv();
                let Ok(index) = job else { break };
                let value = if cancelled() {
                    Err(JobError::Cancelled)
                } else {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map(index))) {
                        Ok(result) => result.map_err(JobError::Task),
                        Err(_) => Err(JobError::Panic),
                    }
                };
                if results.send((index, value)).is_err() {
                    break;
                }
            });
        }
        drop(results);
        let result = (|| {
            for start in (0..total).step_by(config.batch_size) {
                if cancelled() {
                    return Err(JobError::Cancelled);
                }
                let end = total.min(start.saturating_add(config.batch_size));
                for index in start..end {
                    send.send(index).map_err(|_| JobError::Panic)?;
                }
                let mut batch: Vec<_> = (start..end)
                    .map(|_| output.recv().map_err(|_| JobError::Panic))
                    .collect::<Result<_, _>>()?;
                batch.sort_unstable_by_key(|(index, _)| *index);
                for (index, value) in batch {
                    if cancelled() {
                        return Err(JobError::Cancelled);
                    }
                    let value = value?;
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        consume(index, value)
                    })) {
                        Ok(result) => result.map_err(JobError::Task)?,
                        Err(_) => return Err(JobError::Panic),
                    }
                }
            }
            Ok(())
        })();
        // Closing both channels also releases workers after coordinator failure.
        drop(send);
        drop(output);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    const CONFIG: Config = Config {
        workers: 4,
        batch_size: 8,
    };
    #[test]
    fn stress_is_ordered_and_concurrency_is_bounded() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let mut values = Vec::new();
        let consumed = AtomicUsize::new(0);
        ordered_map(
            10_000,
            CONFIG,
            || false,
            |i| {
                assert!(i < consumed.load(Ordering::SeqCst) + CONFIG.batch_size);
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::yield_now();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, ()>(i * i)
            },
            |i, value| {
                assert_eq!(i, values.len());
                values.push(value);
                consumed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(values.len(), 10_000);
        assert!((1..=4).contains(&peak.load(Ordering::SeqCst)));
    }
    #[test]
    fn cancellation_failure_and_panic_join_workers() {
        let cancel = AtomicBool::new(false);
        let mut count = 0;
        assert_eq!(
            ordered_map(
                100,
                CONFIG,
                || cancel.load(Ordering::SeqCst),
                Ok::<_, ()>,
                |_, _| {
                    count += 1;
                    cancel.store(true, Ordering::SeqCst);
                    Ok(())
                }
            ),
            Err(JobError::Cancelled)
        );
        assert_eq!(count, 1);
        assert_eq!(
            ordered_map(
                100,
                CONFIG,
                || false,
                |i| if i == 3 { Err("bad") } else { Ok(i) },
                |_, _| Ok(())
            ),
            Err(JobError::Task("bad"))
        );
        assert_eq!(
            ordered_map(
                100,
                CONFIG,
                || false,
                |i| {
                    if i == 3 {
                        panic!("task");
                    }
                    Ok::<_, ()>(i)
                },
                |_, _| Ok(())
            ),
            Err(JobError::Panic)
        );
        assert_eq!(
            ordered_map(100, CONFIG, || false, Ok::<_, &str>, |_, _| Err("consumer")),
            Err(JobError::Task("consumer"))
        );
        assert_eq!(
            ordered_map(
                100,
                CONFIG,
                || false,
                Ok::<_, ()>,
                |_, _| panic!("consumer")
            ),
            Err(JobError::Panic)
        );
    }
}
