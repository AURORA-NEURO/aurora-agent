"""Choreography P32 throughput inference protocol-compilation integrity feature."""
from .protocol_execution_integrity_support import ProtocolExecutionCard7,ProtocolExecutionRequest4,ProtocolExecutionIntegrityError,manifest,execute
FEATURE_ID="AFA-choreography-P32-F03";CONTRACT_VERSION="choreography-throughput_protocol_execution_integrity_inference/1.0"
def throughput_protocol_execution_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
def execute_throughput_protocol_execution_integrity_inference(request:ProtocolExecutionRequest4)->ProtocolExecutionCard7:return execute(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_protocol_execution_integrity_inference_manifest","execute_throughput_protocol_execution_integrity_inference"]
