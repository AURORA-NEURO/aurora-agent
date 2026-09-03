"""Worldgen P03-F29 local context-compilation federated control plane."""
from .worldgen_context_control_plane_support import ContextControlAttestation, ContextControlPlaneRequest, ContextControlPlaneReceipt, manifest, control
FEATURE_ID="AFA-worldgen-P03-F29"; CONTRACT_VERSION="worldgen-local-context-control-plane/1.0"
def worldgen_local_context_compilation_federated_control_plane_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ContextControlPlaneRequest1@1",scale="local single-study",autonomy_tier="A1")
def control_worldgen_local_context_compilation(request): return control(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,require_federation=False)
__all__=["ContextControlAttestation","ContextControlPlaneRequest","ContextControlPlaneReceipt","worldgen_local_context_compilation_federated_control_plane_manifest","control_worldgen_local_context_compilation"]
