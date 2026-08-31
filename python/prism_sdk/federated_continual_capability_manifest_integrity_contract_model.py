"""Weave P32 local inference protocol-compilation integrity feature."""
from .capability_manifest_integrity_support import CapabilityManifestCard7,CapabilityManifestRequest4,CapabilityManifestIntegrityError,manifest,admit
FEATURE_ID="AFA-weave-P32-F01";CONTRACT_VERSION="weave-local_capability_manifest_integrity_inference/1.0"
def local_capability_manifest_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def admit_local_capability_manifest_integrity_inference(request:CapabilityManifestRequest4)->CapabilityManifestCard7:return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_capability_manifest_integrity_inference_manifest","admit_local_capability_manifest_integrity_inference"]
