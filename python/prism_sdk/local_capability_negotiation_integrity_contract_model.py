"""Backends P32 local contract_model capability-negotiation integrity feature."""
from .capability_negotiation_integrity_support import BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError,manifest,negotiate
FEATURE_ID="AFA-backends-P32-F05";CONTRACT_VERSION="backends-local_capability_negotiation_integrity_contract_model/1.0"
def local_capability_negotiation_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def negotiate_local_capability_negotiation_integrity_contract_model(request:BackendRequest4)->BackendCard7:return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_capability_negotiation_integrity_contract_model_manifest","negotiate_local_capability_negotiation_integrity_contract_model"]
