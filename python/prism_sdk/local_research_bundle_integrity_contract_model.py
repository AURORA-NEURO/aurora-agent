"""Bundle P32 local contract_model signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F05";CONTRACT_VERSION="bundle-local_research_bundle_integrity_contract_model/1.0"
def local_research_bundle_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def release_local_research_bundle_integrity_contract_model(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_research_bundle_integrity_contract_model_manifest","release_local_research_bundle_integrity_contract_model"]
