"""Worldgen P28 throughput semantic parity contract model surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F07"; CONTRACT_VERSION="worldgen-throughput-semantic-parity-contract_model/1.0"
def worldgen_throughput_semantic_parity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")
def compare_worldgen_throughput_semantic_parity_contract(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")

