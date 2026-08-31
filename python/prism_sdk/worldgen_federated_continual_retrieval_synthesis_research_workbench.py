"""Federated continual retrieval-synthesis researcher workbench (AFA-worldgen-P02-F20)."""
from .worldgen_retrieval_workbench_support import RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt, render, manifest
FEATURE_ID="AFA-worldgen-P02-F20"; CONTRACT_VERSION="worldgen-federated-continual-retrieval-synthesis-workbench/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery4@1"
def worldgen_federated_continual_retrieval_synthesis_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual",autonomy_tier="A2")
def render_worldgen_federated_continual_retrieval_synthesis_research_workbench(request:RetrievalWorkbenchRequest)->RetrievalWorkbenchReceipt: return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["RetrievalWorkbenchRequest","RetrievalWorkbenchReceipt","render_worldgen_federated_continual_retrieval_synthesis_research_workbench","worldgen_federated_continual_retrieval_synthesis_research_workbench_manifest"]
