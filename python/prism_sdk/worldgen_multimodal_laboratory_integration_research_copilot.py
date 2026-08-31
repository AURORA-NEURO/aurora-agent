from .worldgen_laboratory_integration_copilot_support import InstrumentCopilotRequest, InstrumentCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P11-F10"; CONTRACT_VERSION="worldgen-multimodal-laboratory_integration-copilot/1.0"
def worldgen_multimodal_laboratory_integration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_laboratory_integration_research_copilot(request:InstrumentCopilotRequest)->InstrumentCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
