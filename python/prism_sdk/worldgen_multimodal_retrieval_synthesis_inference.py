"""AFA-worldgen-P02-F02 multimodal multi-study retrieval and synthesis inference."""
from .worldgen_retrieval_support import RetrievalCandidate, RetrievalQuery, RetrievalReceipt, infer, manifest

FEATURE_ID = "AFA-worldgen-P02-F02"
CONTRACT_VERSION = "worldgen-multimodal-retrieval-synthesis-inference/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery2@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"
SCALE = "multimodal multi-study"

def worldgen_multimodal_retrieval_synthesis_inference_manifest():
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema=INPUT_SCHEMA, scale=SCALE, autonomy_tier="A1")

def infer_worldgen_multimodal_retrieval_synthesis(query: RetrievalQuery) -> RetrievalReceipt:
    return infer(query, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION)

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "SCALE", "RetrievalCandidate", "RetrievalQuery", "RetrievalReceipt", "worldgen_multimodal_retrieval_synthesis_inference_manifest", "infer_worldgen_multimodal_retrieval_synthesis"]
