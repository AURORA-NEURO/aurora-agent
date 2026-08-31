"""Worldgen P23 multimodal evaluation/observability inference surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F02"; CONTRACT_VERSION="worldgen-multimodal-evaluation-observability/1.0"
def worldgen_multimodal_evaluation_observability_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")
def evaluate_worldgen_multimodal_evaluation_observability_inference(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")

