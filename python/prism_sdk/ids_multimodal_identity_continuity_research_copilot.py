"""IDs P32 multimodal multi-study research copilot surface (F10)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F10"; CONTRACT_VERSION="ids-multimodal-identity-continuity-research_copilot/1.0"
def ids_multimodal_identity_continuity_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def qualify_ids_multimodal_identity_continuity_copilot(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
