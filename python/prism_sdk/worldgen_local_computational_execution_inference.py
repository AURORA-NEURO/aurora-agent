from typing import Mapping,Any
from .worldgen_computational_execution_support import ExecutionRun7, assure_computational_execution, manifest
FEATURE_ID="AFA-worldgen-P12-F01"; CONTRACT_VERSION="worldgen-local-computational_execution/1.0"
def worldgen_local_computational_execution_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def assure_computational_execution_worldgen_local_computational_executions(request:Mapping[str,Any])->ExecutionRun7: return assure_computational_execution(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)
