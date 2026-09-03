"""Worldgen P03-F23 prospective high-throughput context-compilation interoperability gateway."""
from .worldgen_context_interoperability_support import ContextInteroperabilityRequest, ContextInteroperabilityReceipt, manifest, negotiate
FEATURE_ID="AFA-worldgen-P03-F23"; CONTRACT_VERSION="worldgen-throughput-context-compilation-gateway/1.0"
def worldgen_throughput_context_compilation_interoperability_gateway_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextCompilationRequest1@1",scale="prospective high-throughput",autonomy_tier="A2")
def negotiate_worldgen_throughput_context_compilation_interoperability(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,require_federation=True)
__all__=["ContextInteroperabilityRequest","ContextInteroperabilityReceipt","worldgen_throughput_context_compilation_interoperability_gateway_manifest","negotiate_worldgen_throughput_context_compilation_interoperability"]
