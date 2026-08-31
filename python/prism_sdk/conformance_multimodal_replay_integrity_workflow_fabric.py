"""Conformance P32 multimodal multi-study workflow_fabric replay-integrity feature F08."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F08";CONTRACT_VERSION="conformance-multimodal-replay-integrity-workflow_fabric/1.0"
def conformance_multimodal_replay_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
def qualify_conformance_multimodal_replay_integrity_workflow_fabric(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
