use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum TraceError {
    #[error("step {step} appears more than once; the producer and the importer disagree about ordering")]
    DuplicateStep { step: usize },

    #[error("step {step} follows step {after}; a trace must be monotonic in step index")]
    NonMonotonicStep { step: usize, after: usize },

    #[error("step {step} names causal parent {parent}, which is not earlier than it")]
    CausalParentNotEarlier { step: usize, parent: usize },

    #[error("step {step} is not present in this trace")]
    StepNotInTrace { step: usize },

    /// Gate 2 requires a named reviewer. An unattributed approval is indistinguishable from none.
    #[error("a cell proposal cannot be approved without a named reviewer")]
    UnattributedApproval,
}
