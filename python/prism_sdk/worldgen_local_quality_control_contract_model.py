from .worldgen_quality_contract_support import QualityContractRequest, QualityContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P07-F05"; CONTRACT_VERSION="worldgen-local-quality-contract/1.0"
def worldgen_local_quality_control_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityContractRequest1@1",scale="local single-study",autonomy_tier="A0")
def negotiate_worldgen_local_quality_contract(request:QualityContractRequest)->QualityContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=false)
