"""Lab P32 local contract_model instrument-execution integrity feature F05."""
from .instrument_execution_integrity_support import InstrumentExecutionRequest4,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-lab-P32-F05";CONTRACT_VERSION="lab-local_instrument_execution_integrity_contract_model/1.0"
def local_instrument_execution_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def qualify_local_instrument_execution_integrity_contract_model(request:InstrumentExecutionRequest4)->InstrumentExecutionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
