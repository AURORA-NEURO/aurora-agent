"""Adapter P32 local contract-model gateway-integrity feature F02."""
from .adapter_gateway_integrity_support import GatewayIntegrityRequest4,GatewayIntegrityCard7,GatewayIntegrityError,manifest,qualify
FEATURE_ID="AFA-adapter-P32-F02";CONTRACT_VERSION="adapter-local_gateway_integrity_contract_model/1.0"
def adapter_local_gateway_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract-model")
def qualify_adapter_local_gateway_integrity_contract_model(request:GatewayIntegrityRequest4)->GatewayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract-model")
