"""Lab P32 local inference instrument-execution integrity feature F01."""
from .instrument_execution_integrity_support import InstrumentExecutionRequest4,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-lab-P32-F01";CONTRACT_VERSION="lab-local_instrument_execution_integrity_inference/1.0"
def local_instrument_execution_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def qualify_local_instrument_execution_integrity_inference(request:InstrumentExecutionRequest4)->InstrumentExecutionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
