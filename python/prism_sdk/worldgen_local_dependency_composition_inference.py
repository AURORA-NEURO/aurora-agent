"""Worldgen P27 local dependency composition inference surface."""
from .worldgen_dependency_composition_support import *
FEATURE_ID="AFA-worldgen-P27-F01"; CONTRACT_VERSION="worldgen-local-dependency-composition-inference/1.0"
def worldgen_local_dependency_composition_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def compose_worldgen_local_dependency_composition(request): return compose(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

