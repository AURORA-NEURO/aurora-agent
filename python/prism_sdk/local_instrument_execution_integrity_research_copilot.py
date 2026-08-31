"""Lab P32 local research_copilot instrument-execution integrity feature F09."""
from .instrument_execution_integrity_support import InstrumentExecutionRequest4,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-lab-P32-F09";CONTRACT_VERSION="lab-local_instrument_execution_integrity_research_copilot/1.0"
def local_instrument_execution_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def qualify_local_instrument_execution_integrity_research_copilot(request:InstrumentExecutionRequest4)->InstrumentExecutionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
