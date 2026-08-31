"""Factory P32 multimodal lease/fencing integrity feature."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F02"; CONTRACT_VERSION="factory-multimodal_lease_fencing_integrity_inference/1.0"
def multimodal_lease_fencing_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
def qualify_multimodal_lease_fencing_integrity_inference(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="inference")
