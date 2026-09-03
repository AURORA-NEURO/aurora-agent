"""Worldgen P21 local performance/reliability contract-model surface."""
from .worldgen_performance_reliability_contract_support import *
FEATURE_ID="AFA-worldgen-P21-F05"; CONTRACT_VERSION="worldgen-local-performance-reliability-contract/1.0"
def worldgen_local_performance_reliability_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_performance_reliability_contract(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
