"""Metrics P32 multimodal workflow_fabric discovery-rate integrity feature F14."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F14";CONTRACT_VERSION="metrics-multimodal_discovery_rate_integrity_workflow_fabric/1.0"
def multimodal_discovery_rate_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="workflow_fabric")
def qualify_multimodal_discovery_rate_integrity_workflow_fabric(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="workflow_fabric")
