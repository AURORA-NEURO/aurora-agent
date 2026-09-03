from .worldgen_experiment_design_contract_support import ExperimentDesignContractRequest, ExperimentDesignContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P09-F05"; CONTRACT_VERSION="worldgen-local-experiment_design-contract/1.0"
def worldgen_local_experiment_design_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExperimentDesignContractRequest1@1",scale="local single-study",autonomy_tier="A0")
def negotiate_worldgen_local_experiment_design_contract(request:ExperimentDesignContractRequest)->ExperimentDesignContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
