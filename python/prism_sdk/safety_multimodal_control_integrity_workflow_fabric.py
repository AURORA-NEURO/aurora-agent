"""Safety P32 multimodal multi-study workflow_fabric control-integrity feature F08."""
from .safety_control_integrity_support import SafetyIntegrityRequest4,SafetyIntegrityCard7,SafetyIntegrityError,manifest,qualify
FEATURE_ID="AFA-safety-P32-F08"; CONTRACT_VERSION="safety-multimodal-control-integrity-workflow_fabric/1.0"
def safety_multimodal_control_integrity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
def qualify_safety_multimodal_control_integrity_workflow_fabric(request:SafetyIntegrityRequest4)->SafetyIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
