"""Factory P32 local lease/fencing integrity feature."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F01"; CONTRACT_VERSION="factory-local_lease_fencing_integrity_inference/1.0"
def local_lease_fencing_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def qualify_local_lease_fencing_integrity_inference(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
