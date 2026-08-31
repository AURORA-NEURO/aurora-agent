"""Backends P32 local workflow_fabric capability-negotiation integrity feature."""
from .capability_negotiation_integrity_support import BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError,manifest,negotiate
FEATURE_ID="AFA-backends-P32-F13";CONTRACT_VERSION="backends-local_capability_negotiation_integrity_workflow_fabric/1.0"
def local_capability_negotiation_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def negotiate_local_capability_negotiation_integrity_workflow_fabric(request:BackendRequest4)->BackendCard7:return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_capability_negotiation_integrity_workflow_fabric_manifest","negotiate_local_capability_negotiation_integrity_workflow_fabric"]
