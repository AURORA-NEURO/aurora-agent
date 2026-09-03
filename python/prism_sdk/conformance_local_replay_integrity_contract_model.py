"""Conformance P32 local single-study contract_model replay-integrity feature F02."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F02";CONTRACT_VERSION="conformance-local-replay-integrity-contract_model/1.0"
def conformance_local_replay_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
def qualify_conformance_local_replay_integrity_contract_model(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
