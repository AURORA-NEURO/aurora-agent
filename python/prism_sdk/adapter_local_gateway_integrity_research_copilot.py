"""Adapter P32 local research-copilot gateway-integrity feature F03."""
from .adapter_gateway_integrity_support import GatewayIntegrityRequest4,GatewayIntegrityCard7,GatewayIntegrityError,manifest,qualify
FEATURE_ID="AFA-adapter-P32-F03";CONTRACT_VERSION="adapter-local_gateway_integrity_research_copilot/1.0"
def adapter_local_gateway_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research-copilot")
def qualify_adapter_local_gateway_integrity_research_copilot(request:GatewayIntegrityRequest4)->GatewayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research-copilot")
