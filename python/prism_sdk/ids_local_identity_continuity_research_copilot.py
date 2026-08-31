"""IDs P32 local single-study research copilot surface (F09)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F09"; CONTRACT_VERSION="ids-local-identity-continuity-research_copilot/1.0"
def ids_local_identity_continuity_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def qualify_ids_local_identity_continuity_copilot(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
