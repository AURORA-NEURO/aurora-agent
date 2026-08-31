"""AFA-worldgen-P02-F03 prospective high-throughput retrieval and synthesis inference."""
from .worldgen_retrieval_support import RetrievalCandidate, RetrievalQuery, RetrievalReceipt, infer, manifest

FEATURE_ID = "AFA-worldgen-P02-F03"
CONTRACT_VERSION = "worldgen-throughput-retrieval-synthesis-inference/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"
SCALE = "prospective high-throughput"

def worldgen_throughput_retrieval_synthesis_inference_manifest():
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema=INPUT_SCHEMA, scale=SCALE, autonomy_tier="A1")

def infer_worldgen_throughput_retrieval_synthesis(query: RetrievalQuery) -> RetrievalReceipt:
    return infer(query, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION)

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "SCALE", "RetrievalCandidate", "RetrievalQuery", "RetrievalReceipt", "worldgen_throughput_retrieval_synthesis_inference_manifest", "infer_worldgen_throughput_retrieval_synthesis"]
