"""Metrics P32 local inference discovery-rate integrity feature F01."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F01";CONTRACT_VERSION="metrics-local_discovery_rate_integrity_inference/1.0"
def local_discovery_rate_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def qualify_local_discovery_rate_integrity_inference(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
