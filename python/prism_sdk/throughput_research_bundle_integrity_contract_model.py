"""Bundle P32 throughput contract_model signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F07";CONTRACT_VERSION="bundle-throughput_research_bundle_integrity_contract_model/1.0"
def throughput_research_bundle_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="contract_model")
def release_throughput_research_bundle_integrity_contract_model(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="contract_model")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_research_bundle_integrity_contract_model_manifest","release_throughput_research_bundle_integrity_contract_model"]
