"""Worldgen P27 local dependency composition contract model surface."""
from .worldgen_dependency_composition_support import *
FEATURE_ID="AFA-worldgen-P27-F05"; CONTRACT_VERSION="worldgen-local-dependency-composition-contract_model/1.0"
def worldgen_local_dependency_composition_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def compose_worldgen_local_dependency_composition_contract(request): return compose(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

