"""Safety P32 local single-study research_copilot control-integrity feature F03."""
from .safety_control_integrity_support import SafetyIntegrityRequest4,SafetyIntegrityCard7,SafetyIntegrityError,manifest,qualify
FEATURE_ID="AFA-safety-P32-F03"; CONTRACT_VERSION="safety-local-control-integrity-research_copilot/1.0"
def safety_local_control_integrity_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
def qualify_safety_local_control_integrity_research_copilot(request:SafetyIntegrityRequest4)->SafetyIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
