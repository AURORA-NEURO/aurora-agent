"""Worldgen P03-F17 local context-compilation researcher workbench."""
from .worldgen_context_workbench_support import ContextWorkbenchRequest, ContextWorkbenchReceipt, manifest, render
FEATURE_ID="AFA-worldgen-P03-F17"; CONTRACT_VERSION="worldgen-local-context-compilation-workbench/1.0"
def worldgen_local_context_compilation_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCopilotRequest1@1",scale="local single-study",autonomy_tier="A1")
def render_worldgen_local_context_compilation_research_workbench(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["ContextWorkbenchRequest","ContextWorkbenchReceipt","worldgen_local_context_compilation_research_workbench_manifest","render_worldgen_local_context_compilation_research_workbench"]
