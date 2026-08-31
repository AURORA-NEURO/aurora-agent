"""AFA-worldgen-P02-F05 local retrieval-synthesis contract model."""
from .worldgen_retrieval_contract_support import RetrievalCandidate, RetrievalContractRequest, RetrievalContractReceipt, compile_contract, manifest
FEATURE_ID="AFA-worldgen-P02-F05"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-contract/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"; OUTPUT_SCHEMA="EvidenceSynthesis2@1"; SCALE="local single-study"
def worldgen_local_retrieval_synthesis_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A0")
def compile_worldgen_local_retrieval_synthesis_contract(request): return compile_contract(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,expected_input_schema=INPUT_SCHEMA)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","RetrievalCandidate","RetrievalContractRequest","RetrievalContractReceipt","worldgen_local_retrieval_synthesis_contract_model_manifest","compile_worldgen_local_retrieval_synthesis_contract"]
