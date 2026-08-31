"""Bundle P32 throughput research_copilot signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F11";CONTRACT_VERSION="bundle-throughput_research_bundle_integrity_research_copilot/1.0"
def throughput_research_bundle_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="research_copilot")
def release_throughput_research_bundle_integrity_research_copilot(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="research_copilot")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_research_bundle_integrity_research_copilot_manifest","release_throughput_research_bundle_integrity_research_copilot"]
