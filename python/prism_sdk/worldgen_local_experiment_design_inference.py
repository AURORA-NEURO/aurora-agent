from .worldgen_experiment_design_support import ExperimentDesignQuestion, ExperimentDesignPortfolio, design, manifest
FEATURE_ID="AFA-worldgen-P09-F01"; CONTRACT_VERSION="worldgen-local-experiment_design-design_request/1.0"
def worldgen_local_experiment_design_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignQuestion1@1",scale="local single-study",autonomy_tier="A0")
def design_worldgen_local_experiment_designs(request:ExperimentDesignQuestion)->ExperimentDesignPortfolio: return design(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
