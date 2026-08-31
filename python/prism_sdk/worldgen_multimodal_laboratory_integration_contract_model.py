from .worldgen_laboratory_integration_contract_support import InstrumentContractRequest, InstrumentContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P11-F06"; CONTRACT_VERSION="worldgen-multimodal-laboratory_integration-contract/1.0"
def worldgen_multimodal_laboratory_integration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentContractRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def negotiate_worldgen_multimodal_laboratory_integration_contract(request:InstrumentContractRequest)->InstrumentContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=False)
