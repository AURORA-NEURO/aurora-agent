from .worldgen_experiment_design_copilot_support import ExperimentDesignCopilotRequest, ExperimentDesignCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P09-F10"; CONTRACT_VERSION="worldgen-multimodal-experiment_design-copilot/1.0"
def worldgen_multimodal_experiment_design_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_experiment_design_research_copilot(request:ExperimentDesignCopilotRequest)->ExperimentDesignCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
