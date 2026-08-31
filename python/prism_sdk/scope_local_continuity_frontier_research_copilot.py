"""Scope P32 local single-study research copilot surface (F09)."""
from .scope_continuity_frontier_support import *
FEATURE_ID="AFA-scope-P32-F09"; CONTRACT_VERSION="scope-local-continuity-frontier-research_copilot/1.0"
def scope_local_continuity_frontier_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def qualify_scope_local_continuity_frontier_copilot(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
