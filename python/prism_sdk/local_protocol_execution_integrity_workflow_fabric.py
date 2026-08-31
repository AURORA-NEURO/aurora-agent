"""Choreography P32 local workflow_fabric protocol-compilation integrity feature."""
from .protocol_execution_integrity_support import ProtocolExecutionCard7,ProtocolExecutionRequest4,ProtocolExecutionIntegrityError,manifest,execute
FEATURE_ID="AFA-choreography-P32-F13";CONTRACT_VERSION="choreography-local_protocol_execution_integrity_workflow_fabric/1.0"
def local_protocol_execution_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def execute_local_protocol_execution_integrity_workflow_fabric(request:ProtocolExecutionRequest4)->ProtocolExecutionCard7:return execute(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_protocol_execution_integrity_workflow_fabric_manifest","execute_local_protocol_execution_integrity_workflow_fabric"]
