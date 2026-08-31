"""Worldgen P03-F32 federated continual context-compilation federated control plane."""
from .worldgen_context_control_plane_support import ContextControlAttestation, ContextControlPlaneRequest, ContextControlPlaneReceipt, manifest, control
FEATURE_ID="AFA-worldgen-P03-F32"; CONTRACT_VERSION="worldgen-federated-continual-context-control-plane/1.0"
def worldgen_federated_continual_context_compilation_federated_control_plane_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextControlPlaneRequest1@1",scale="federated continual autonomous",autonomy_tier="A2")
def control_worldgen_federated_continual_context_compilation(request): return control(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,require_federation=True)
__all__=["ContextControlAttestation","ContextControlPlaneRequest","ContextControlPlaneReceipt","worldgen_federated_continual_context_compilation_federated_control_plane_manifest","control_worldgen_federated_continual_context_compilation"]
