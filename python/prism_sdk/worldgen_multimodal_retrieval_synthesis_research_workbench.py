"""Multimodal retrieval-synthesis researcher workbench (AFA-worldgen-P02-F18)."""
from .worldgen_retrieval_workbench_support import RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt, render, manifest
FEATURE_ID="AFA-worldgen-P02-F18"; CONTRACT_VERSION="worldgen-multimodal-retrieval-synthesis-workbench/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery2@1"
def worldgen_multimodal_retrieval_synthesis_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def render_worldgen_multimodal_retrieval_synthesis_research_workbench(request:RetrievalWorkbenchRequest)->RetrievalWorkbenchReceipt: return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["RetrievalWorkbenchRequest","RetrievalWorkbenchReceipt","render_worldgen_multimodal_retrieval_synthesis_research_workbench","worldgen_multimodal_retrieval_synthesis_research_workbench_manifest"]
