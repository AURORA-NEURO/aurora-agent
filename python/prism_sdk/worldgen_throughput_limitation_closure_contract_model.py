"""Worldgen P26 throughput limitation closure contract model surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F07"; CONTRACT_VERSION="worldgen-throughput-limitation-closure-contract_model/1.0"
def worldgen_throughput_limitation_closure_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")
def close_worldgen_throughput_limitation_closure_contract(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")

