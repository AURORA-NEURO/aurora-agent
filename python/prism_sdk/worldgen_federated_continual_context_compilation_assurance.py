"""Worldgen P03-F28 federated continual context-compilation assurance harness."""
from .worldgen_context_assurance_support import ContextAssuranceRequest, ContextAssuranceReceipt, manifest, assure
FEATURE_ID="AFA-worldgen-P03-F28"; CONTRACT_VERSION="worldgen-federated-continual-context-assurance/1.0"
def worldgen_federated_continual_context_compilation_assurance_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextAssuranceRequest1@1",scale="federated continual autonomous",autonomy_tier="A2")
def assure_worldgen_federated_continual_context_compilation(request): return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,require_federation=True)
__all__=["ContextAssuranceRequest","ContextAssuranceReceipt","worldgen_federated_continual_context_compilation_assurance_manifest","assure_worldgen_federated_continual_context_compilation"]
