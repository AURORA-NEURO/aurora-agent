"""Infra P32 local research-copilot reliability-integrity feature F03."""
from .infra_reliability_integrity_support import ReliabilityIntegrityRequest4,ReliabilityIntegrityCard7,ReliabilityIntegrityError,manifest,qualify
FEATURE_ID="AFA-infra-P32-F03";CONTRACT_VERSION="infra-local_reliability_integrity_research_copilot/1.0"
def infra_local_reliability_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research-copilot")
def qualify_infra_local_reliability_integrity_research_copilot(request:ReliabilityIntegrityRequest4)->ReliabilityIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research-copilot")
