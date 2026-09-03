"""Scope P32 local single-study contract model surface (F05)."""
from .scope_continuity_frontier_support import *
FEATURE_ID="AFA-scope-P32-F05"; CONTRACT_VERSION="scope-local-continuity-frontier-contract_model/1.0"
def scope_local_continuity_frontier_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def qualify_scope_local_continuity_frontier_contract(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
