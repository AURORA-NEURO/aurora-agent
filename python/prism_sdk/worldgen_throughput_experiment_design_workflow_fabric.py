from .worldgen_experiment_design_workflow_support import ExperimentDesignWorkflowRequest, ExperimentDesignWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P09-F15"; CONTRACT_VERSION="worldgen-throughput-experiment_design-workflow/1.0"
def worldgen_throughput_experiment_design_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignWorkflowRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def schedule_worldgen_throughput_experiment_design_workflow(request:ExperimentDesignWorkflowRequest)->ExperimentDesignWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=false,require_federation=False)
