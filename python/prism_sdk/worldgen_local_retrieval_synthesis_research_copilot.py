"""AFA-worldgen-P02-F09 local retrieval-synthesis research copilot."""
from .worldgen_retrieval_copilot_support import RetrievalCopilotRequest, RetrievalCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P02-F09"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-copilot/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"; OUTPUT_SCHEMA="EvidenceSynthesis3@1"; SCALE="local single-study"
def worldgen_local_retrieval_synthesis_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A1")
def run_worldgen_local_retrieval_synthesis_research_copilot(request): return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","RetrievalCopilotRequest","RetrievalCopilotReceipt","worldgen_local_retrieval_synthesis_research_copilot_manifest","run_worldgen_local_retrieval_synthesis_research_copilot"]
