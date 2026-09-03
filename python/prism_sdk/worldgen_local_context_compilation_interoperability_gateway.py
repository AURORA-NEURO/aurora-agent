"""Worldgen P03-F21 local context-compilation interoperability gateway."""
from .worldgen_context_interoperability_support import ContextInteroperabilityRequest, ContextInteroperabilityReceipt, manifest, negotiate
FEATURE_ID="AFA-worldgen-P03-F21"; CONTRACT_VERSION="worldgen-local-context-compilation-gateway/1.0"
def worldgen_local_context_compilation_interoperability_gateway_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCompilationRequest1@1",scale="local single-study",autonomy_tier="A1")
def negotiate_worldgen_local_context_compilation_interoperability(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,require_federation=False)
__all__=["ContextInteroperabilityRequest","ContextInteroperabilityReceipt","worldgen_local_context_compilation_interoperability_gateway_manifest","negotiate_worldgen_local_context_compilation_interoperability"]
