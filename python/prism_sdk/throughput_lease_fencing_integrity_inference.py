"""Factory P32 throughput lease/fencing integrity feature."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F03"; CONTRACT_VERSION="factory-throughput_lease_fencing_integrity_inference/1.0"
def throughput_lease_fencing_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
def qualify_throughput_lease_fencing_integrity_inference(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
