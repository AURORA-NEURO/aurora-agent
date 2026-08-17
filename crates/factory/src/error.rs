use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FactoryError {
    #[error("job {job_id} is already present")]
    DuplicateJobId { job_id: String },

    #[error("no such job: {job_id}")]
    UnknownJob { job_id: String },

    #[error("job {job_id} has no active lease")]
    NoActiveLease { job_id: String },

    /// Invariant 1: one active lease per attempt.
    #[error("job {job_id} is leased by {holder}, not by the caller")]
    LeaseHeldByAnother { job_id: String, holder: String },

    #[error("the lease on job {job_id} has expired; the store may already have recovered it")]
    LeaseExpired { job_id: String },

    /// The caller presented an older attempt after the job was recovered and fenced.
    #[error("stale lease fence for job {job_id}: expected attempt {expected_attempt}, active attempt {active_attempt}")]
    StaleLease {
        job_id: String,
        expected_attempt: u32,
        active_attempt: u32,
    },

    /// Invariant 3: a success with nothing staged would record a job as succeeded with no output.
    #[error("job {job_id} reported success but staged no output")]
    NothingStaged { job_id: String },

    #[error("job {job_id} is not compensable")]
    NotCompensable { job_id: String },

    #[error("job {job_id} is not awaiting compensation")]
    NotAwaitingCompensation { job_id: String },

    #[error("job {job_id} is not quarantined")]
    NotQuarantined { job_id: String },

    /// Releasing a quarantined non-idempotent job is a human decision and must be attributable.
    #[error("releasing job {job_id} from quarantine requires a named operator")]
    UnattributedRelease { job_id: String },

    #[error("job {job_id} is already terminal ({state})")]
    AlreadyTerminal { job_id: String, state: String },

    #[error("queue admission policy is invalid: {reason}")]
    InvalidAdmissionPolicy { reason: String },

    #[error("queue admission limit {dimension} is {limit}; observed {observed}")]
    AdmissionLimit {
        dimension: String,
        limit: usize,
        observed: usize,
    },

    #[error("job-store snapshot is invalid: {reason}")]
    InvalidSnapshot { reason: String },

    #[error("job-store snapshot digest mismatch: expected {expected}, computed {actual}")]
    SnapshotDigestMismatch { expected: String, actual: String },

    #[error("job-store snapshot is {bytes} bytes, above the {max_bytes}-byte bound")]
    SnapshotTooLarge { bytes: usize, max_bytes: usize },

    #[error("job-store snapshot {operation} failed for {path}: {reason}")]
    SnapshotIo {
        operation: String,
        path: String,
        reason: String,
    },

    #[error("job-store snapshot could not be serialized: {reason}")]
    SnapshotSerialization { reason: String },
}
