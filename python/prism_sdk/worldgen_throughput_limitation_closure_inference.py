"""Worldgen P26 throughput limitation closure inference surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F03"; CONTRACT_VERSION="worldgen-throughput-limitation-closure-inference/1.0"
def worldgen_throughput_limitation_closure_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")
def close_worldgen_throughput_limitation_closure(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")

