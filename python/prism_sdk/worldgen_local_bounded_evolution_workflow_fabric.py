"""Worldgen P32 local single-study workflow fabric surface (F13)."""
from .worldgen_bounded_evolution_support import *
FEATURE_ID="AFA-worldgen-P32-F13"; CONTRACT_VERSION="worldgen-local-bounded-evolution-workflow_fabric/1.0"
def worldgen_local_bounded_evolution_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def promote_worldgen_local_bounded_evolution_workflow(request): return promote(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
