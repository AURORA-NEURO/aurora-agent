"""Worldgen P25 throughput contract-frontier contract model surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F07"; CONTRACT_VERSION="worldgen-throughput-contract-frontier-contract_model/1.0"
def worldgen_throughput_contract_frontier_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")
def admit_worldgen_throughput_contract_frontier_contract(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")

