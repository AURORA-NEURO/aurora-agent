"""Worldgen P03-F18 multimodal context-compilation researcher workbench."""
from .worldgen_context_workbench_support import ContextWorkbenchRequest, ContextWorkbenchReceipt, manifest, render
FEATURE_ID="AFA-worldgen-P03-F18"; CONTRACT_VERSION="worldgen-multimodal-context-compilation-workbench/1.0"
def worldgen_multimodal_context_compilation_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def render_worldgen_multimodal_context_compilation_research_workbench(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["ContextWorkbenchRequest","ContextWorkbenchReceipt","worldgen_multimodal_context_compilation_research_workbench_manifest","render_worldgen_multimodal_context_compilation_research_workbench"]
