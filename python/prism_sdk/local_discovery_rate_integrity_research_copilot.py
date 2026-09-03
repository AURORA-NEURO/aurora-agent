"""Metrics P32 local research_copilot discovery-rate integrity feature F09."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F09";CONTRACT_VERSION="metrics-local_discovery_rate_integrity_research_copilot/1.0"
def local_discovery_rate_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def qualify_local_discovery_rate_integrity_research_copilot(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
