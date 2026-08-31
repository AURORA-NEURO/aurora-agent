"""Worldgen P27 local dependency composition workflow fabric surface."""
from .worldgen_dependency_composition_support import *
FEATURE_ID="AFA-worldgen-P27-F13"; CONTRACT_VERSION="worldgen-local-dependency-composition-workflow_fabric/1.0"
def worldgen_local_dependency_composition_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def compose_worldgen_local_dependency_composition_workflow(request): return compose(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

