"""IDs P32 local single-study workflow fabric surface (F13)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F13"; CONTRACT_VERSION="ids-local-identity-continuity-workflow_fabric/1.0"
def ids_local_identity_continuity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def qualify_ids_local_identity_continuity_workflow(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
