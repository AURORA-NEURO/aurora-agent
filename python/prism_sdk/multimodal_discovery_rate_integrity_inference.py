"""Metrics P32 multimodal inference discovery-rate integrity feature F02."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F02";CONTRACT_VERSION="metrics-multimodal_discovery_rate_integrity_inference/1.0"
def multimodal_discovery_rate_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
def qualify_multimodal_discovery_rate_integrity_inference(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
