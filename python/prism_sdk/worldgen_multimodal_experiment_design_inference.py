from .worldgen_experiment_design_support import ExperimentDesignQuestion, ExperimentDesignPortfolio, design, manifest
FEATURE_ID="AFA-worldgen-P09-F02"; CONTRACT_VERSION="worldgen-multimodal-experiment_design-design_request/1.0"
def worldgen_multimodal_experiment_design_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignQuestion1@1",scale="multimodal multi-study",autonomy_tier="A1")
def design_worldgen_multimodal_experiment_designs(request:ExperimentDesignQuestion)->ExperimentDesignPortfolio: return design(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=False)
