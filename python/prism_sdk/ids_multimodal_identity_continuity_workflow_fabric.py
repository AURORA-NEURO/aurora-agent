"""IDs P32 multimodal multi-study workflow fabric surface (F14)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F14"; CONTRACT_VERSION="ids-multimodal-identity-continuity-workflow_fabric/1.0"
def ids_multimodal_identity_continuity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")
def qualify_ids_multimodal_identity_continuity_workflow(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")
