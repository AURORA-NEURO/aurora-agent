"""AFA-worldgen-P01-F32 federated continual operations service."""
from .worldgen_operations_service import OperationsEvent, OperationsRequest, OperationsReceipt, manifest, operate
FEATURE_ID="AFA-worldgen-P01-F32"; CONTRACT_VERSION="worldgen-federated-continual-evidence-surveillance-operations/1.0"; INPUT_SCHEMA="EvidenceFeed4@1"; OUTPUT_SCHEMA="QualifiedEvidenceSet8@1"; SCALE="federated continual autonomous"
def worldgen_federated_continual_evidence_surveillance_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,output_schema=OUTPUT_SCHEMA,scale=SCALE,autonomy_tier="A2")
def operate_worldgen_federated_continual_evidence_surveillance(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","OperationsEvent","OperationsRequest","OperationsReceipt","worldgen_federated_continual_evidence_surveillance_operations_manifest","operate_worldgen_federated_continual_evidence_surveillance"]
