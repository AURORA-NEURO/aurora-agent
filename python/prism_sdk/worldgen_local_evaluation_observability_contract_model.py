"""Worldgen P23 local evaluation/observability contract model surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F05"; CONTRACT_VERSION="worldgen-local-evaluation-observability-contract/1.0"
def worldgen_local_evaluation_observability_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def negotiate_worldgen_local_evaluation_observability_contract_model(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

