"""Bundle P32 multimodal inference signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F02";CONTRACT_VERSION="bundle-multimodal_research_bundle_integrity_inference/1.0"
def multimodal_research_bundle_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
def release_multimodal_research_bundle_integrity_inference(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","multimodal_research_bundle_integrity_inference_manifest","release_multimodal_research_bundle_integrity_inference"]
