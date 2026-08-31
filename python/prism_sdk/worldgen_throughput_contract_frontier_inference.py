"""Worldgen P25 throughput contract-frontier inference surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F03"; CONTRACT_VERSION="worldgen-throughput-contract-frontier-inference/1.0"
def worldgen_throughput_contract_frontier_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")
def admit_worldgen_throughput_contract_frontier(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")

