"""Safety P32 local single-study contract_model control-integrity feature F02."""
from .safety_control_integrity_support import SafetyIntegrityRequest4,SafetyIntegrityCard7,SafetyIntegrityError,manifest,qualify
FEATURE_ID="AFA-safety-P32-F02"; CONTRACT_VERSION="safety-local-control-integrity-contract_model/1.0"
def safety_local_control_integrity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
def qualify_safety_local_control_integrity_contract_model(request:SafetyIntegrityRequest4)->SafetyIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
