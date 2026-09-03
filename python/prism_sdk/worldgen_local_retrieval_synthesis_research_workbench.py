"""Local retrieval-synthesis researcher workbench (AFA-worldgen-P02-F17)."""
from .worldgen_retrieval_workbench_support import RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt, render, manifest
FEATURE_ID="AFA-worldgen-P02-F17"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-workbench/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"
def worldgen_local_retrieval_synthesis_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="local single-study",autonomy_tier="A0")
def render_worldgen_local_retrieval_synthesis_research_workbench(request:RetrievalWorkbenchRequest)->RetrievalWorkbenchReceipt: return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["RetrievalWorkbenchRequest","RetrievalWorkbenchReceipt","render_worldgen_local_retrieval_synthesis_research_workbench","worldgen_local_retrieval_synthesis_research_workbench_manifest"]
