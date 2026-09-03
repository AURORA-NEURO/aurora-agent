//! Cancellation by generation counter.
//!
//! A cancellation is a monotone bump of a task's generation. Dispatch jobs carry the generation
//! they were created under; a worker re-checking before execution sees `is_cancelled == true`
//! when any newer generation exists. There is no "cancel succeeded" boolean that could lie about
//! an in-flight attempt: cooperative cancellation means the *next* observation point observes
//! it, and receipts record whether a cancel raced the run.

use crate::ids::TaskId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct CancelRegistry {
    generations: BTreeMap<TaskId, u64>,
}

impl CancelRegistry {
    /// Requests cancellation; returns true if this call initiated it.
    pub fn cancel(&mut self, task: TaskId) -> bool {
        let g = self.generations.entry(task).or_insert(0);
        *g += 1;
        true
    }

    pub fn is_requested(&self, task: TaskId) -> bool {
        self.generations.contains_key(&task)
    }

    pub fn generation(&self, task: TaskId) -> u64 {
        self.generations.get(&task).copied().unwrap_or(0)
    }
}

/// Shared handle used by drivers and the scheduler. Lock hold times are O(log n) map ops —
/// never held across handler execution.
#[derive(Clone, Debug, Default)]
pub struct CancelState(Arc<Mutex<CancelRegistry>>);

impl CancelState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self, task: TaskId) -> bool {
        self.0.lock().expect("cancel lock").cancel(task)
    }

    pub fn snapshot(&self, task: TaskId) -> u64 {
        self.0.lock().expect("cancel lock").generation(task)
    }

    pub fn is_cancelled_since(&self, task: TaskId, seen_gen: u64) -> bool {
        let reg = self.0.lock().expect("cancel lock");
        reg.generation(task) > seen_gen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TaskId;

    fn task(n: u64) -> TaskId {
        TaskId::from_raw(n).expect("nonzero")
    }

    #[test]
    fn generations_move_forward_only_and_cancel_races_are_detectable() {
        let s = CancelState::new();
        let t = task(1);
        let gen = s.snapshot(t);
        assert_eq!(gen, 0);
        assert!(!s.is_cancelled_since(t, gen));
        assert!(s.cancel(t));
        assert!(
            s.is_cancelled_since(t, gen),
            "job dispatched under gen 0 now sees cancel"
        );
        assert_eq!(s.snapshot(t), 1);
        // A job that was already dispatched under the new generation is not retroactively
        // cancelled again by observing the same state.
        assert!(!s.is_cancelled_since(t, 1));
    }

    #[test]
    fn cancelling_an_unknown_task_is_a_no_op_request_not_an_error() {
        let s = CancelState::new();
        assert!(s.cancel(task(42)));
        assert!(s.is_cancelled_since(task(42), 0));
    }
}
