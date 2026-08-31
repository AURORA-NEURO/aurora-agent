"""AFA-worldgen-P02-F04 federated continual retrieval and synthesis inference."""
from .worldgen_retrieval_support import RetrievalCandidate, RetrievalQuery, RetrievalReceipt, infer, manifest

FEATURE_ID = "AFA-worldgen-P02-F04"
CONTRACT_VERSION = "worldgen-federated-continual-retrieval-synthesis-inference/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery4@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"
SCALE = "federated continual autonomous"

def worldgen_federated_continual_retrieval_synthesis_inference_manifest():
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema=INPUT_SCHEMA, scale=SCALE, autonomy_tier="A1")

def infer_worldgen_federated_continual_retrieval_synthesis(query: RetrievalQuery) -> RetrievalReceipt:
    return infer(query, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION)

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "SCALE", "RetrievalCandidate", "RetrievalQuery", "RetrievalReceipt", "worldgen_federated_continual_retrieval_synthesis_inference_manifest", "infer_worldgen_federated_continual_retrieval_synthesis"]
