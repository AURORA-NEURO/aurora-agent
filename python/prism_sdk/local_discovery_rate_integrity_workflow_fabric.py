"""Metrics P32 local workflow_fabric discovery-rate integrity feature F13."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F13";CONTRACT_VERSION="metrics-local_discovery_rate_integrity_workflow_fabric/1.0"
def local_discovery_rate_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_discovery_rate_integrity_workflow_fabric(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
