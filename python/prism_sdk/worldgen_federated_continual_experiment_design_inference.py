from .worldgen_experiment_design_support import ExperimentDesignQuestion, ExperimentDesignPortfolio, design, manifest
FEATURE_ID="AFA-worldgen-P09-F04"; CONTRACT_VERSION="worldgen-federated_continual-experiment_design-design_request/1.0"
def worldgen_federated_continual_experiment_design_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignQuestion1@1",scale="federated continual autonomous",autonomy_tier="A1")
def design_worldgen_federated_continual_experiment_designs(request:ExperimentDesignQuestion)->ExperimentDesignPortfolio: return design(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=True)
