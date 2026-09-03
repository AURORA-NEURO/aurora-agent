"""Factory P32 local lease/fencing integrity research copilot."""
from .lease_fencing_integrity_support import *
FEATURE_ID="AFA-factory-P32-F09"; CONTRACT_VERSION="factory-local_lease_fencing_integrity_research_copilot/1.0"
def local_lease_fencing_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def qualify_local_lease_fencing_integrity_research_copilot(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
