"""AFA-worldgen-P02-F11 prospective throughput retrieval-synthesis research copilot."""
from .worldgen_retrieval_copilot_support import RetrievalCopilotRequest, RetrievalCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P02-F11"; CONTRACT_VERSION="worldgen-throughput-retrieval-synthesis-copilot/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery3@1"; OUTPUT_SCHEMA="EvidenceSynthesis3@1"; SCALE="prospective high-throughput"
def worldgen_throughput_retrieval_synthesis_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A2")
def run_worldgen_throughput_retrieval_synthesis_research_copilot(request): return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","RetrievalCopilotRequest","RetrievalCopilotReceipt","worldgen_throughput_retrieval_synthesis_research_copilot_manifest","run_worldgen_throughput_retrieval_synthesis_research_copilot"]
