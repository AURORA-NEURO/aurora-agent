"""Safety P32 local single-study workflow_fabric control-integrity feature F04."""
from .safety_control_integrity_support import SafetyIntegrityRequest4,SafetyIntegrityCard7,SafetyIntegrityError,manifest,qualify
FEATURE_ID="AFA-safety-P32-F04"; CONTRACT_VERSION="safety-local-control-integrity-workflow_fabric/1.0"
def safety_local_control_integrity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
def qualify_safety_local_control_integrity_workflow_fabric(request:SafetyIntegrityRequest4)->SafetyIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
