from .worldgen_laboratory_integration_contract_support import InstrumentContractRequest, InstrumentContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P11-F07"; CONTRACT_VERSION="worldgen-throughput-laboratory_integration-contract/1.0"
def worldgen_throughput_laboratory_integration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentContractRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def negotiate_worldgen_throughput_laboratory_integration_contract(request:InstrumentContractRequest)->InstrumentContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=False)
