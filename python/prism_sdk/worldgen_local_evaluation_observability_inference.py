"""Worldgen P23 local evaluation/observability inference surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F01"; CONTRACT_VERSION="worldgen-local-evaluation-observability/1.0"
def worldgen_local_evaluation_observability_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def evaluate_worldgen_local_evaluation_observability_inference(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

