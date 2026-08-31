from .worldgen_experiment_design_workflow_support import ExperimentDesignWorkflowRequest, ExperimentDesignWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P09-F13"; CONTRACT_VERSION="worldgen-local-experiment_design-workflow/1.0"
def worldgen_local_experiment_design_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_experiment_design_workflow(request:ExperimentDesignWorkflowRequest)->ExperimentDesignWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=true,require_federation=False)
