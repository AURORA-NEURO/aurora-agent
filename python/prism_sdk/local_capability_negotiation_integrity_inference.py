"""Backends P32 local inference capability-negotiation integrity feature."""
from .capability_negotiation_integrity_support import BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError,manifest,negotiate
FEATURE_ID="AFA-backends-P32-F01";CONTRACT_VERSION="backends-local_capability_negotiation_integrity_inference/1.0"
def local_capability_negotiation_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def negotiate_local_capability_negotiation_integrity_inference(request:BackendRequest4)->BackendCard7:return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_capability_negotiation_integrity_inference_manifest","negotiate_local_capability_negotiation_integrity_inference"]
