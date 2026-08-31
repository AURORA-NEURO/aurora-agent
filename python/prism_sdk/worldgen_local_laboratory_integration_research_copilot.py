from .worldgen_laboratory_integration_copilot_support import InstrumentCopilotRequest, InstrumentCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P11-F09"; CONTRACT_VERSION="worldgen-local-laboratory_integration-copilot/1.0"
def worldgen_local_laboratory_integration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_laboratory_integration_research_copilot(request:InstrumentCopilotRequest)->InstrumentCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
