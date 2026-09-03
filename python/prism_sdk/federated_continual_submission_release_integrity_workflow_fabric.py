"""Hub P32 local inference protocol-compilation integrity feature."""
from .submission_release_integrity_support import SubmissionReleaseCard7,SubmissionReleaseRequest4,SubmissionReleaseIntegrityError,manifest,release
FEATURE_ID="AFA-hub-P32-F01";CONTRACT_VERSION="hub-local_submission_release_integrity_inference/1.0"
def local_submission_release_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def release_local_submission_release_integrity_inference(request:SubmissionReleaseRequest4)->SubmissionReleaseCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_submission_release_integrity_inference_manifest","release_local_submission_release_integrity_inference"]
