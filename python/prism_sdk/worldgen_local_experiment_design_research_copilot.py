from .worldgen_experiment_design_copilot_support import ExperimentDesignCopilotRequest, ExperimentDesignCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P09-F09"; CONTRACT_VERSION="worldgen-local-experiment_design-copilot/1.0"
def worldgen_local_experiment_design_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_experiment_design_research_copilot(request:ExperimentDesignCopilotRequest)->ExperimentDesignCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
