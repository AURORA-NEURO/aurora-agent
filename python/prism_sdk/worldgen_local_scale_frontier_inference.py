"""Worldgen P29 local scale frontier inference surface."""
from .worldgen_scale_frontier_support import *
FEATURE_ID="AFA-worldgen-P29-F01"; CONTRACT_VERSION="worldgen-local-scale-frontier-inference/1.0"
def worldgen_local_scale_frontier_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def evaluate_worldgen_local_scale_frontier(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

