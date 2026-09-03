"""Scope P32 local single-study workflow fabric surface (F13)."""
from .scope_continuity_frontier_support import *
FEATURE_ID="AFA-scope-P32-F13"; CONTRACT_VERSION="scope-local-continuity-frontier-workflow_fabric/1.0"
def scope_local_continuity_frontier_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def qualify_scope_local_continuity_frontier_workflow(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
