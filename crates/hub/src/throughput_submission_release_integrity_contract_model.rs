//! Hub P32 local inference submission-release integrity feature.
use super::submission_release_integrity_support::{
    manifest, release, SubmissionReleaseCard7, SubmissionReleaseIntegrityError,
    SubmissionReleaseRequest4,
};
pub const FEATURE_ID: &str = "AFA-hub-P32-F01";
pub const CONTRACT_VERSION: &str = "hub-local_submission_release_integrity_inference/1.0";
pub fn local_submission_release_integrity_inference_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
pub fn release_local_submission_release_integrity_inference(
    request: &SubmissionReleaseRequest4,
) -> Result<SubmissionReleaseCard7, SubmissionReleaseIntegrityError> {
    release(request, FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
