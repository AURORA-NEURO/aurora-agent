"""Bundle P32 throughput workflow_fabric signed research-object integrity feature."""
from .research_bundle_integrity_support import BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError,manifest,release
FEATURE_ID="AFA-bundle-P32-F15";CONTRACT_VERSION="bundle-throughput_research_bundle_integrity_workflow_fabric/1.0"
def throughput_research_bundle_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="workflow_fabric")
def release_throughput_research_bundle_integrity_workflow_fabric(request:BundleReleaseRequest4)->BundleCard7:return release(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="workflow_fabric")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_research_bundle_integrity_workflow_fabric_manifest","release_throughput_research_bundle_integrity_workflow_fabric"]
