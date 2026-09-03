"""Worldgen P23 throughput evaluation/observability inference surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F03"; CONTRACT_VERSION="worldgen-throughput-evaluation-observability/1.0"
def worldgen_throughput_evaluation_observability_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")
def evaluate_worldgen_throughput_evaluation_observability_inference(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")

