"""Adapter P32 local workflow-fabric gateway-integrity feature F04."""
from .adapter_gateway_integrity_support import GatewayIntegrityRequest4,GatewayIntegrityCard7,GatewayIntegrityError,manifest,qualify
FEATURE_ID="AFA-adapter-P32-F04";CONTRACT_VERSION="adapter-local_gateway_integrity_workflow_fabric/1.0"
def adapter_local_gateway_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
def qualify_adapter_local_gateway_integrity_workflow_fabric(request:GatewayIntegrityRequest4)->GatewayIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
