from .worldgen_experiment_design_copilot_support import ExperimentDesignCopilotRequest, ExperimentDesignCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P09-F11"; CONTRACT_VERSION="worldgen-throughput-experiment_design-copilot/1.0"
def worldgen_throughput_experiment_design_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignCopilotRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def run_worldgen_throughput_experiment_design_research_copilot(request:ExperimentDesignCopilotRequest)->ExperimentDesignCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=False,require_federation=False)
