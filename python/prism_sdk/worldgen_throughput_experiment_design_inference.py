from .worldgen_experiment_design_support import ExperimentDesignQuestion, ExperimentDesignPortfolio, design, manifest
FEATURE_ID="AFA-worldgen-P09-F03"; CONTRACT_VERSION="worldgen-throughput-experiment_design-design_request/1.0"
def worldgen_throughput_experiment_design_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignQuestion1@1",scale="prospective high-throughput",autonomy_tier="A1")
def design_worldgen_throughput_experiment_designs(request:ExperimentDesignQuestion)->ExperimentDesignPortfolio: return design(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=False)
