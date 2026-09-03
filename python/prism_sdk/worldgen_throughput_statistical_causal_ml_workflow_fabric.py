from .worldgen_statistical_causal_ml_workflow_support import *
FEATURE_ID="AFA-worldgen-P13-F15"; CONTRACT_VERSION="worldgen-throughput-statistical-causal-ml-workflow/1.0"
def worldgen_throughput_statistical_causal_ml_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_statistical_causal_ml_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

