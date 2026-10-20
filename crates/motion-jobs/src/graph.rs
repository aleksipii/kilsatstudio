//! Dependency-ordered jobs on a fixed pool of at most four scoped workers.
//!
//! Jobs are registered by name with declared dependencies. `run` validates the graph
//! (duplicate names, unknown dependencies, cycles) before executing anything. The
//! coordinator (the calling thread) dispatches ready jobs in registration order with at
//! most `workers` jobs in flight, so the queue is bounded and no thread is spawned per
//! job. Each job receives the outputs of its dependencies in declaration order.
//! Failure, panic and cancellation propagate to dependants as typed outcomes; running
//! jobs cannot be interrupted and must observe their own cancellation.
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{mpsc, Arc, Mutex},
};

pub const MAX_GRAPH_JOBS: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum GraphError {
    InvalidWorkers,
    TooManyJobs,
    DuplicateName(String),
    UnknownDependency { job: String, dependency: String },
    Cycle(Vec<String>),
}
impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorkers => write!(f, "worker count must be between 1 and 4"),
            Self::TooManyJobs => write!(f, "job graph exceeds {MAX_GRAPH_JOBS} jobs"),
            Self::DuplicateName(n) => write!(f, "job name `{n}` is registered twice"),
            Self::UnknownDependency { job, dependency } => {
                write!(f, "job `{job}` depends on unknown job `{dependency}`")
            }
            Self::Cycle(names) => write!(f, "dependency cycle among jobs: {}", names.join(", ")),
        }
    }
}
impl std::error::Error for GraphError {}

/// Result of one job after the graph finished.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome<T, E> {
    Done(Arc<T>),
    Failed(E),
    Panicked,
    /// Cancellation was observed (or a dependency was cancelled) before the job started.
    Cancelled,
    /// The named dependency failed, panicked, was cancelled or was itself skipped.
    DependencyFailed(String),
}
impl<T, E> Outcome<T, E> {
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done(_))
    }
}

/// `(name, outcome)` per job in registration order.
pub type Results<T, E> = Vec<(String, Outcome<T, E>)>;
type JobFn<'a, T, E> = Box<dyn FnOnce(&[Arc<T>]) -> Result<T, E> + Send + 'a>;
struct Node<'a, T, E> {
    name: String,
    deps: Vec<String>,
    job: Option<JobFn<'a, T, E>>,
}

pub struct Graph<'a, T, E> {
    nodes: Vec<Node<'a, T, E>>,
}
impl<T, E> Default for Graph<'_, T, E> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl<'a, T: Send + Sync, E: Send> Graph<'a, T, E> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn add(
        &mut self,
        name: &str,
        deps: &[&str],
        job: impl FnOnce(&[Arc<T>]) -> Result<T, E> + Send + 'a,
    ) -> Result<(), GraphError> {
        if self.nodes.len() >= MAX_GRAPH_JOBS {
            return Err(GraphError::TooManyJobs);
        }
        if self.nodes.iter().any(|n| n.name == name) {
            return Err(GraphError::DuplicateName(name.into()));
        }
        self.nodes.push(Node {
            name: name.into(),
            deps: deps.iter().map(|d| d.to_string()).collect(),
            job: Some(Box::new(job)),
        });
        Ok(())
    }

    /// Resolve dependency names to indices and reject unknown names and cycles.
    fn resolve(&self) -> Result<Vec<Vec<usize>>, GraphError> {
        let index: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();
        let mut deps = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let mut resolved = Vec::with_capacity(node.deps.len());
            for d in &node.deps {
                resolved.push(*index.get(d.as_str()).ok_or_else(|| {
                    GraphError::UnknownDependency {
                        job: node.name.clone(),
                        dependency: d.clone(),
                    }
                })?);
            }
            deps.push(resolved);
        }
        // Kahn: anything left unvisited is on or downstream of a cycle.
        let mut remaining: Vec<usize> = deps.iter().map(|d| d.len()).collect();
        let mut dependants = vec![Vec::new(); deps.len()];
        for (i, d) in deps.iter().enumerate() {
            for &p in d {
                dependants[p].push(i);
            }
        }
        let mut queue: VecDeque<usize> = (0..deps.len()).filter(|&i| remaining[i] == 0).collect();
        let mut visited = 0;
        while let Some(i) = queue.pop_front() {
            visited += 1;
            for &c in &dependants[i] {
                remaining[c] -= 1;
                if remaining[c] == 0 {
                    queue.push_back(c);
                }
            }
        }
        if visited != deps.len() {
            return Err(GraphError::Cycle(
                (0..deps.len())
                    .filter(|&i| remaining[i] > 0)
                    .map(|i| self.nodes[i].name.clone())
                    .collect(),
            ));
        }
        Ok(deps)
    }

    /// Execute the graph. Returns `(name, outcome)` in registration order.
    pub fn run(
        mut self,
        workers: usize,
        cancelled: impl Fn() -> bool + Sync,
    ) -> Result<Results<T, E>, GraphError> {
        if !(1..=4).contains(&workers) {
            return Err(GraphError::InvalidWorkers);
        }
        let deps = self.resolve()?;
        let count = self.nodes.len();
        let mut dependants = vec![Vec::new(); count];
        for (i, d) in deps.iter().enumerate() {
            for &p in d {
                dependants[p].push(i);
            }
        }
        let mut remaining: Vec<usize> = deps.iter().map(|d| d.len()).collect();
        let mut outcomes: Vec<Option<Outcome<T, E>>> = (0..count).map(|_| None).collect();
        let mut ready: VecDeque<usize> = (0..count).filter(|&i| remaining[i] == 0).collect();
        let mut finished = 0usize;
        let mut in_flight = 0usize;

        type Work<'a, T, E> = (usize, JobFn<'a, T, E>, Vec<Arc<T>>);
        let (send, receive) = mpsc::sync_channel::<Work<'a, T, E>>(workers);
        let receive = Mutex::new(receive);
        let (results, output) = mpsc::sync_channel::<(usize, Outcome<T, E>)>(workers);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let receive = &receive;
                let results = results.clone();
                scope.spawn(move || loop {
                    let work = receive.lock().expect("job receiver poisoned").recv();
                    let Ok((index, job, inputs)) = work else {
                        break;
                    };
                    let outcome = match catch_unwind(AssertUnwindSafe(|| job(&inputs))) {
                        Ok(Ok(value)) => Outcome::Done(Arc::new(value)),
                        Ok(Err(error)) => Outcome::Failed(error),
                        Err(_) => Outcome::Panicked,
                    };
                    if results.send((index, outcome)).is_err() {
                        break;
                    }
                });
            }
            drop(results);

            // Mark `start` with `first` and everything downstream as not run.
            let settle = |outcomes: &mut Vec<Option<Outcome<T, E>>>,
                          finished: &mut usize,
                          start: usize,
                          cancel: bool,
                          names: &[Node<'a, T, E>]| {
                let mut stack = vec![(start, names[start].name.clone())];
                while let Some((parent, cause)) = stack.pop() {
                    for &child in &dependants[parent] {
                        if outcomes[child].is_none() {
                            outcomes[child] = Some(if cancel {
                                Outcome::Cancelled
                            } else {
                                Outcome::DependencyFailed(cause.clone())
                            });
                            *finished += 1;
                            stack.push((child, cause.clone()));
                        }
                    }
                }
            };

            while finished < count {
                while in_flight < workers {
                    let Some(i) = ready.pop_front() else { break };
                    if outcomes[i].is_some() {
                        continue;
                    }
                    if cancelled() {
                        outcomes[i] = Some(Outcome::Cancelled);
                        finished += 1;
                        settle(&mut outcomes, &mut finished, i, true, &self.nodes);
                        continue;
                    }
                    let inputs = deps[i]
                        .iter()
                        .map(|&p| match &outcomes[p] {
                            Some(Outcome::Done(v)) => v.clone(),
                            _ => unreachable!("dependency finished successfully before dispatch"),
                        })
                        .collect();
                    let job = self.nodes[i].job.take().expect("job dispatched once");
                    send.send((i, job, inputs))
                        .expect("workers outlive dispatch");
                    in_flight += 1;
                }
                if in_flight == 0 {
                    // Nothing running and nothing ready: all remaining jobs are settled.
                    break;
                }
                let (i, outcome) = output.recv().expect("workers outlive results");
                in_flight -= 1;
                finished += 1;
                let done = outcome.is_done();
                outcomes[i] = Some(outcome);
                if done {
                    for &c in &dependants[i] {
                        if outcomes[c].is_none() {
                            remaining[c] -= 1;
                            if remaining[c] == 0 {
                                ready.push_back(c);
                            }
                        }
                    }
                } else {
                    settle(&mut outcomes, &mut finished, i, false, &self.nodes);
                }
            }
            drop(send);
        });
        Ok(self
            .nodes
            .into_iter()
            .zip(outcomes)
            .map(|(n, o)| (n.name, o.unwrap_or(Outcome::Cancelled)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    type G<'a> = Graph<'a, u32, &'static str>;
    fn outcome<'a>(
        r: &'a [(String, Outcome<u32, &'static str>)],
        n: &str,
    ) -> &'a Outcome<u32, &'static str> {
        &r.iter().find(|(name, _)| name == n).unwrap().1
    }

    #[test]
    fn dependencies_run_first_and_receive_outputs_in_declared_order() {
        let order = Mutex::new(Vec::new());
        let mut g = G::new();
        // Registered out of order: forward references are allowed.
        g.add("sum", &["b", "a"], |i| {
            order.lock().unwrap().push("sum");
            Ok(*i[0] * 10 + *i[1])
        })
        .unwrap();
        g.add("a", &[], |_| {
            order.lock().unwrap().push("a");
            Ok(1)
        })
        .unwrap();
        g.add("b", &["a"], |i| {
            order.lock().unwrap().push("b");
            Ok(*i[0] + 1)
        })
        .unwrap();
        let r = g.run(3, || false).unwrap();
        assert_eq!(outcome(&r, "sum"), &Outcome::Done(Arc::new(21)));
        assert_eq!(*order.lock().unwrap(), ["a", "b", "sum"]);
        assert_eq!(
            r.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["sum", "a", "b"]
        );
    }

    #[test]
    fn invalid_graphs_are_rejected_before_any_job_runs() {
        let ran = AtomicBool::new(false);
        let mut g = G::new();
        g.add("ok", &[], |_| {
            ran.store(true, Ordering::SeqCst);
            Ok(0)
        })
        .unwrap();
        assert_eq!(
            g.add("ok", &[], |_| Ok(0)),
            Err(GraphError::DuplicateName("ok".into()))
        );
        g.add("x", &["missing"], |_| Ok(0)).unwrap();
        assert_eq!(
            g.run(2, || false).unwrap_err(),
            GraphError::UnknownDependency {
                job: "x".into(),
                dependency: "missing".into()
            }
        );
        let mut g = G::new();
        g.add("a", &["c"], |_| Ok(0)).unwrap();
        g.add("b", &["a"], |_| Ok(0)).unwrap();
        g.add("c", &["b"], |_| Ok(0)).unwrap();
        g.add("free", &[], |_| Ok(0)).unwrap();
        g.add("self", &["self"], |_| Ok(0)).unwrap();
        match g.run(2, || false).unwrap_err() {
            GraphError::Cycle(names) => {
                for n in ["a", "b", "c", "self"] {
                    assert!(names.contains(&n.to_string()));
                }
                assert!(!names.contains(&"free".to_string()));
            }
            other => panic!("{other:?}"),
        }
        assert!(!ran.load(Ordering::SeqCst));
        assert_eq!(
            G::new().run(0, || false).unwrap_err(),
            GraphError::InvalidWorkers
        );
        assert_eq!(
            G::new().run(5, || false).unwrap_err(),
            GraphError::InvalidWorkers
        );
    }

    #[test]
    fn failure_and_panic_propagate_to_dependants_only() {
        let mut g = G::new();
        g.add("bad", &[], |_| Err("boom")).unwrap();
        g.add("boom", &[], |_| panic!("task")).unwrap();
        g.add("child", &["bad"], |_| Ok(1)).unwrap();
        g.add("grandchild", &["child"], |_| Ok(1)).unwrap();
        g.add("panic_child", &["boom", "bad"], |_| Ok(1)).unwrap();
        g.add("independent", &[], |_| Ok(7)).unwrap();
        let r = g.run(2, || false).unwrap();
        assert_eq!(outcome(&r, "bad"), &Outcome::Failed("boom"));
        assert_eq!(outcome(&r, "boom"), &Outcome::Panicked);
        assert_eq!(
            outcome(&r, "child"),
            &Outcome::DependencyFailed("bad".into())
        );
        assert_eq!(
            outcome(&r, "grandchild"),
            &Outcome::DependencyFailed("bad".into())
        );
        assert!(matches!(
            outcome(&r, "panic_child"),
            Outcome::DependencyFailed(_)
        ));
        assert_eq!(outcome(&r, "independent"), &Outcome::Done(Arc::new(7)));
    }

    #[test]
    fn cancellation_stops_unstarted_jobs_and_their_dependants() {
        let cancel = AtomicBool::new(false);
        let mut g = G::new();
        g.add("first", &[], |_| {
            cancel.store(true, Ordering::SeqCst);
            Ok(1)
        })
        .unwrap();
        g.add("second", &["first"], |_| Ok(2)).unwrap();
        g.add("third", &["second"], |_| Ok(3)).unwrap();
        let r = g.run(1, || cancel.load(Ordering::SeqCst)).unwrap();
        assert_eq!(outcome(&r, "first"), &Outcome::Done(Arc::new(1)));
        assert_eq!(outcome(&r, "second"), &Outcome::Cancelled);
        assert_eq!(outcome(&r, "third"), &Outcome::Cancelled);
        // Cancelled before anything starts: nothing runs.
        let mut g = G::new();
        g.add("only", &[], |_| panic!("must not run")).unwrap();
        let r = g.run(2, || true).unwrap();
        assert_eq!(outcome(&r, "only"), &Outcome::Cancelled);
    }

    #[test]
    fn concurrency_is_bounded_and_wide_graph_completes() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let mut g = G::new();
        g.add("root", &[], |_| Ok(1)).unwrap();
        let names: Vec<String> = (0..200).map(|i| format!("leaf{i}")).collect();
        for n in &names {
            g.add(n, &["root"], |i| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::yield_now();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(*i[0] + 1)
            })
            .unwrap();
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        g.add("join", &refs, |i| Ok(i.iter().map(|v| **v).sum()))
            .unwrap();
        let r = g.run(4, || false).unwrap();
        assert_eq!(outcome(&r, "join"), &Outcome::Done(Arc::new(400)));
        assert!(peak.load(Ordering::SeqCst) <= 4);
    }
}
