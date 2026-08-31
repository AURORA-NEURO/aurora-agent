"""Worldgen P28 throughput semantic parity inference surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F03"; CONTRACT_VERSION="worldgen-throughput-semantic-parity-inference/1.0"
def worldgen_throughput_semantic_parity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")
def compare_worldgen_throughput_semantic_parity(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")

