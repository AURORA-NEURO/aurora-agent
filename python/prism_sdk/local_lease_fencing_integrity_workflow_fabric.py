"""Factory P32 local lease/fencing integrity workflow fabric."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F13"; CONTRACT_VERSION="factory-local_lease_fencing_integrity_workflow_fabric/1.0"
def local_lease_fencing_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_lease_fencing_integrity_workflow_fabric(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
