"""Worldgen P26 local limitation closure workflow fabric surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F13"; CONTRACT_VERSION="worldgen-local-limitation-closure-workflow_fabric/1.0"
def worldgen_local_limitation_closure_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def close_worldgen_local_limitation_closure_workflow(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

