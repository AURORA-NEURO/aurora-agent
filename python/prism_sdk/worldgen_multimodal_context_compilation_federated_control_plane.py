"""Worldgen P03-F30 multimodal context-compilation federated control plane."""
from .worldgen_context_control_plane_support import ContextControlAttestation, ContextControlPlaneRequest, ContextControlPlaneReceipt, manifest, control
FEATURE_ID="AFA-worldgen-P03-F30"; CONTRACT_VERSION="worldgen-multimodal-context-control-plane/1.0"
def worldgen_multimodal_context_compilation_federated_control_plane_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextControlPlaneRequest1@1",scale="multimodal multi-study",autonomy_tier="A2")
def control_worldgen_multimodal_context_compilation(request): return control(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=True,require_federation=False)
__all__=["ContextControlAttestation","ContextControlPlaneRequest","ContextControlPlaneReceipt","worldgen_multimodal_context_compilation_federated_control_plane_manifest","control_worldgen_multimodal_context_compilation"]
