from .worldgen_quality_contract_support import QualityContractRequest, QualityContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P07-F07"; CONTRACT_VERSION="worldgen-throughput-quality-contract/1.0"
def worldgen_throughput_quality_control_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityContractRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def negotiate_worldgen_throughput_quality_contract(request:QualityContractRequest)->QualityContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=false)
