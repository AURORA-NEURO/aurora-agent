"""Conformance P32 multimodal multi-study contract_model replay-integrity feature F06."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F06";CONTRACT_VERSION="conformance-multimodal-replay-integrity-contract_model/1.0"
def conformance_multimodal_replay_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract_model")
def qualify_conformance_multimodal_replay_integrity_contract_model(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract_model")
