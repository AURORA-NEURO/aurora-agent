"""Backends P32 local research_copilot capability-negotiation integrity feature."""
from .capability_negotiation_integrity_support import BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError,manifest,negotiate
FEATURE_ID="AFA-backends-P32-F09";CONTRACT_VERSION="backends-local_capability_negotiation_integrity_research_copilot/1.0"
def local_capability_negotiation_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def negotiate_local_capability_negotiation_integrity_research_copilot(request:BackendRequest4)->BackendCard7:return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_capability_negotiation_integrity_research_copilot_manifest","negotiate_local_capability_negotiation_integrity_research_copilot"]
