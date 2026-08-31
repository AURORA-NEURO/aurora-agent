"""Factory P32 multimodal lease/fencing integrity contract model."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F06"; CONTRACT_VERSION="factory-multimodal_lease_fencing_integrity_contract_model/1.0"
def multimodal_lease_fencing_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="contract_model")
def qualify_multimodal_lease_fencing_integrity_contract_model(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal",mode="contract_model")
