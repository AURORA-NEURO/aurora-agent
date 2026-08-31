"""Worldgen P21 local performance/reliability inference surface."""
from .worldgen_performance_reliability_support import *
FEATURE_ID="AFA-worldgen-P21-F01"; CONTRACT_VERSION="worldgen-local-performance-reliability/1.0"
def worldgen_local_performance_reliability_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def assess_worldgen_local_performance_reliability(request): return assess(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)
