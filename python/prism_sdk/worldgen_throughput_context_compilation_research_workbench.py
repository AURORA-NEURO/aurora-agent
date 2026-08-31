"""Worldgen P03-F19 prospective high-throughput context-compilation researcher workbench."""
from .worldgen_context_workbench_support import ContextWorkbenchRequest, ContextWorkbenchReceipt, manifest, render
FEATURE_ID="AFA-worldgen-P03-F19"; CONTRACT_VERSION="worldgen-throughput-context-compilation-workbench/1.0"
def worldgen_throughput_context_compilation_research_workbench_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCopilotRequest1@1",scale="prospective high-throughput",autonomy_tier="A2")
def render_worldgen_throughput_context_compilation_research_workbench(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["ContextWorkbenchRequest","ContextWorkbenchReceipt","worldgen_throughput_context_compilation_research_workbench_manifest","render_worldgen_throughput_context_compilation_research_workbench"]
