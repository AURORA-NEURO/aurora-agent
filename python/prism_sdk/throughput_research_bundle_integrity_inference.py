"""Bundle P32 throughput inference signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F03";CONTRACT_VERSION="bundle-throughput_research_bundle_integrity_inference/1.0"
def throughput_research_bundle_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
def release_throughput_research_bundle_integrity_inference(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_research_bundle_integrity_inference_manifest","release_throughput_research_bundle_integrity_inference"]
