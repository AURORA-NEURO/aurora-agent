"""Choreography P32 local inference protocol-compilation integrity feature."""
from .protocol_execution_integrity_support import ProtocolExecutionCard7,ProtocolExecutionRequest4,ProtocolExecutionIntegrityError,manifest,execute
FEATURE_ID="AFA-choreography-P32-F01";CONTRACT_VERSION="choreography-local_protocol_execution_integrity_inference/1.0"
def local_protocol_execution_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def execute_local_protocol_execution_integrity_inference(request:ProtocolExecutionRequest4)->ProtocolExecutionCard7:return execute(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_protocol_execution_integrity_inference_manifest","execute_local_protocol_execution_integrity_inference"]
