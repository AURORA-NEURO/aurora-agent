"""Conformance P32 local single-study research_copilot replay-integrity feature F03."""
from .conformance_replay_integrity_support import ReplayIntegrityRequest4,ReplayIntegrityCard7,ReplayIntegrityError,manifest,qualify
FEATURE_ID="AFA-conformance-P32-F03";CONTRACT_VERSION="conformance-local-replay-integrity-research_copilot/1.0"
def conformance_local_replay_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
def qualify_conformance_local_replay_integrity_research_copilot(request:ReplayIntegrityRequest4)->ReplayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
