"""Factory P32 local lease/fencing integrity contract model."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F05"; CONTRACT_VERSION="factory-local_lease_fencing_integrity_contract_model/1.0"
def local_lease_fencing_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def qualify_local_lease_fencing_integrity_contract_model(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
