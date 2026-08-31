from .worldgen_laboratory_integration_contract_support import InstrumentContractRequest, InstrumentContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P11-F05"; CONTRACT_VERSION="worldgen-local-laboratory_integration-contract/1.0"
def worldgen_local_laboratory_integration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentContractRequest1@1",scale="local single-study",autonomy_tier="A0")
def negotiate_worldgen_local_laboratory_integration_contract(request:InstrumentContractRequest)->InstrumentContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
