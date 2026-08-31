"""Worldgen P03-F22 multimodal context-compilation interoperability gateway."""
from .worldgen_context_interoperability_support import ContextInteroperabilityRequest, ContextInteroperabilityReceipt, manifest, negotiate
FEATURE_ID="AFA-worldgen-P03-F22"; CONTRACT_VERSION="worldgen-multimodal-context-compilation-gateway/1.0"
def worldgen_multimodal_context_compilation_interoperability_gateway_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCompilationRequest1@1",scale="multimodal multi-study",autonomy_tier="A2")
def negotiate_worldgen_multimodal_context_compilation_interoperability(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=True,require_federation=False)
__all__=["ContextInteroperabilityRequest","ContextInteroperabilityReceipt","worldgen_multimodal_context_compilation_interoperability_gateway_manifest","negotiate_worldgen_multimodal_context_compilation_interoperability"]
