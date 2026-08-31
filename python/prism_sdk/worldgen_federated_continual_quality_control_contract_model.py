from .worldgen_quality_contract_support import QualityContractRequest, QualityContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P07-F08"; CONTRACT_VERSION="worldgen-federated_continual-quality-contract/1.0"
def worldgen_federated_continual_quality_control_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityContractRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def negotiate_worldgen_federated_continual_quality_contract(request:QualityContractRequest)->QualityContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=true)
