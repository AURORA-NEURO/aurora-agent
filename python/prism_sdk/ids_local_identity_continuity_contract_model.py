"""IDs P32 local single-study contract model surface (F05)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F05"; CONTRACT_VERSION="ids-local-identity-continuity-contract_model/1.0"
def ids_local_identity_continuity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def qualify_ids_local_identity_continuity_contract(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
