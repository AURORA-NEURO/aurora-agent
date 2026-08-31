"""Choreography P32 local research_copilot protocol-compilation integrity feature."""
from .protocol_execution_integrity_support import ProtocolExecutionCard7,ProtocolExecutionRequest4,ProtocolExecutionIntegrityError,manifest,execute
FEATURE_ID="AFA-choreography-P32-F09";CONTRACT_VERSION="choreography-local_protocol_execution_integrity_research_copilot/1.0"
def local_protocol_execution_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def execute_local_protocol_execution_integrity_research_copilot(request:ProtocolExecutionRequest4)->ProtocolExecutionCard7:return execute(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_protocol_execution_integrity_research_copilot_manifest","execute_local_protocol_execution_integrity_research_copilot"]
