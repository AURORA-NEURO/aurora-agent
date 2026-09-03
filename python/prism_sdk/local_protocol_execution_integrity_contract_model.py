"""Choreography P32 local contract_model protocol-compilation integrity feature."""
from .protocol_execution_integrity_support import ProtocolExecutionCard7,ProtocolExecutionRequest4,ProtocolExecutionIntegrityError,manifest,execute
FEATURE_ID="AFA-choreography-P32-F05";CONTRACT_VERSION="choreography-local_protocol_execution_integrity_contract_model/1.0"
def local_protocol_execution_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def execute_local_protocol_execution_integrity_contract_model(request:ProtocolExecutionRequest4)->ProtocolExecutionCard7:return execute(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_protocol_execution_integrity_contract_model_manifest","execute_local_protocol_execution_integrity_contract_model"]
