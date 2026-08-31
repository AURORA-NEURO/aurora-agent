"""Conformance P32 local single-study inference replay-integrity feature F01."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F01";CONTRACT_VERSION="conformance-local-replay-integrity-inference/1.0"
def conformance_local_replay_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_conformance_local_replay_integrity_inference(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
