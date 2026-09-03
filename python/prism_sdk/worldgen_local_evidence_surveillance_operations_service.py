"""AFA-worldgen-P01-F29 local single-study operations service."""
from .worldgen_operations_service import OperationsEvent, OperationsRequest, OperationsReceipt, manifest, operate
FEATURE_ID="AFA-worldgen-P01-F29"; CONTRACT_VERSION="worldgen-local-evidence-surveillance-operations/1.0"; INPUT_SCHEMA="EvidenceFeed1@1"; OUTPUT_SCHEMA="QualifiedEvidenceSet8@1"; SCALE="local single-study"
def worldgen_local_evidence_surveillance_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,output_schema=OUTPUT_SCHEMA,scale=SCALE,autonomy_tier="A1")
def operate_worldgen_local_evidence_surveillance(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","OperationsEvent","OperationsRequest","OperationsReceipt","worldgen_local_evidence_surveillance_operations_manifest","operate_worldgen_local_evidence_surveillance"]
