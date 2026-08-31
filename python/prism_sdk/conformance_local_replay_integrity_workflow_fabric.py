"""Conformance P32 local single-study workflow_fabric replay-integrity feature F04."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F04";CONTRACT_VERSION="conformance-local-replay-integrity-workflow_fabric/1.0"
def conformance_local_replay_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
def qualify_conformance_local_replay_integrity_workflow_fabric(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
