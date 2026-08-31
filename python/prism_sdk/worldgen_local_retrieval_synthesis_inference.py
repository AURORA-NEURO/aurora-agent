"""AFA-worldgen-P02-F01 local single-study retrieval and synthesis inference."""
from .worldgen_retrieval_support import RetrievalCandidate, RetrievalQuery, RetrievalReceipt, infer, manifest

FEATURE_ID = "AFA-worldgen-P02-F01"
CONTRACT_VERSION = "worldgen-local-retrieval-synthesis-inference/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery1@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"
SCALE = "local single-study"

def worldgen_local_retrieval_synthesis_inference_manifest():
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema=INPUT_SCHEMA, scale=SCALE, autonomy_tier="A0")

def infer_worldgen_local_retrieval_synthesis(query: RetrievalQuery) -> RetrievalReceipt:
    return infer(query, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION)

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "SCALE", "RetrievalCandidate", "RetrievalQuery", "RetrievalReceipt", "worldgen_local_retrieval_synthesis_inference_manifest", "infer_worldgen_local_retrieval_synthesis"]
