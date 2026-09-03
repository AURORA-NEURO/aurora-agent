"""Metrics P32 throughput research_copilot discovery-rate integrity feature F11."""
from .discovery_rate_integrity_support import DiscoveryRateRequest4,DiscoveryRateCard7,DiscoveryRateIntegrityError,manifest,qualify
FEATURE_ID="AFA-metrics-P32-F11";CONTRACT_VERSION="metrics-throughput_discovery_rate_integrity_research_copilot/1.0"
def throughput_discovery_rate_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="research_copilot")
def qualify_throughput_discovery_rate_integrity_research_copilot(request:DiscoveryRateRequest4)->DiscoveryRateCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="research_copilot")
