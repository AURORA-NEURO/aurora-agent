from .worldgen_computational_execution_contract_support import ExecutionContractRequest, ExecutionContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P12-F06"; CONTRACT_VERSION="worldgen-multimodal-computational_execution-contract/1.0"
def worldgen_multimodal_computational_execution_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExecutionContractRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def negotiate_worldgen_multimodal_computational_execution_contract(request:ExecutionContractRequest)->ExecutionContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=False)
