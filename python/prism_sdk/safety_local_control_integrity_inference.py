"""Safety P32 local single-study inference control-integrity feature F01."""
from .safety_control_integrity_support import SafetyIntegrityRequest4,SafetyIntegrityCard7,SafetyIntegrityError,manifest,qualify
FEATURE_ID="AFA-safety-P32-F01"; CONTRACT_VERSION="safety-local-control-integrity-inference/1.0"
def safety_local_control_integrity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_safety_local_control_integrity_inference(request:SafetyIntegrityRequest4)->SafetyIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
