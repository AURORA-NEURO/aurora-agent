"""IDs P32 local single-study inference surface (F01)."""
from .ids_identity_continuity_support import *
FEATURE_ID="AFA-ids-P32-F01"; CONTRACT_VERSION="ids-local-identity-continuity-inference/1.0"
def ids_local_identity_continuity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_ids_local_identity_continuity(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
