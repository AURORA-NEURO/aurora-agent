"""Worldgen P27 throughput dependency composition workflow fabric surface."""
from .worldgen_dependency_composition_support import *
FEATURE_ID="AFA-worldgen-P27-F15"; CONTRACT_VERSION="worldgen-throughput-dependency-composition-workflow_fabric/1.0"
def worldgen_throughput_dependency_composition_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")
def compose_worldgen_throughput_dependency_composition_workflow(request): return compose(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")

