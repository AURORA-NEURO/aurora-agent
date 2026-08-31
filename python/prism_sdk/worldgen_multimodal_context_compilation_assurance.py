"""Worldgen P03-F26 multimodal context-compilation assurance harness."""
from .worldgen_context_assurance_support import ContextAssuranceRequest, ContextAssuranceReceipt, manifest, assure
FEATURE_ID="AFA-worldgen-P03-F26"; CONTRACT_VERSION="worldgen-multimodal-context-assurance/1.0"
def worldgen_multimodal_context_compilation_assurance_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextAssuranceRequest1@1",scale="multimodal multi-study",autonomy_tier="A2")
def assure_worldgen_multimodal_context_compilation(request): return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=True,require_federation=False)
__all__=["ContextAssuranceRequest","ContextAssuranceReceipt","worldgen_multimodal_context_compilation_assurance_manifest","assure_worldgen_multimodal_context_compilation"]
