"""Lab P32 local workflow_fabric instrument-execution integrity feature F13."""
from .instrument_execution_integrity_support import InstrumentExecutionRequest4,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-lab-P32-F13";CONTRACT_VERSION="lab-local_instrument_execution_integrity_workflow_fabric/1.0"
def local_instrument_execution_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_instrument_execution_integrity_workflow_fabric(request:InstrumentExecutionRequest4)->InstrumentExecutionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
