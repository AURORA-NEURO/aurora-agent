"""Metrics P32 throughput workflow_fabric discovery-rate integrity feature F15."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F15";CONTRACT_VERSION="metrics-throughput_discovery_rate_integrity_workflow_fabric/1.0"
def throughput_discovery_rate_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="workflow_fabric")
def qualify_throughput_discovery_rate_integrity_workflow_fabric(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="workflow_fabric")
