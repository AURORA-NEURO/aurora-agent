"""Worldgen P03-F25 local context-compilation assurance harness."""
from .worldgen_context_assurance_support import ContextAssuranceRequest, ContextAssuranceReceipt, manifest, assure
FEATURE_ID="AFA-worldgen-P03-F25"; CONTRACT_VERSION="worldgen-local-context-assurance/1.0"
def worldgen_local_context_compilation_assurance_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextAssuranceRequest1@1",scale="local single-study",autonomy_tier="A1")
def assure_worldgen_local_context_compilation(request): return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,require_federation=False)
__all__=["ContextAssuranceRequest","ContextAssuranceReceipt","worldgen_local_context_compilation_assurance_manifest","assure_worldgen_local_context_compilation"]
