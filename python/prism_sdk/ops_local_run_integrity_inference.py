"""Ops P32 local single-study inference run-integrity feature F01."""
from .ops_run_integrity_support import RunIntegrityRequest4,RunIntegrityCard7,RunIntegrityError,manifest,qualify
FEATURE_ID="AFA-ops-P32-F01";CONTRACT_VERSION="ops-local-run-integrity-inference/1.0"
def ops_local_run_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_ops_local_run_integrity_inference(request:RunIntegrityRequest4)->RunIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
