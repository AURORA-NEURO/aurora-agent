from .worldgen_laboratory_integration_copilot_support import InstrumentCopilotRequest, InstrumentCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P11-F11"; CONTRACT_VERSION="worldgen-throughput-laboratory_integration-copilot/1.0"
def worldgen_throughput_laboratory_integration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentCopilotRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def run_worldgen_throughput_laboratory_integration_research_copilot(request:InstrumentCopilotRequest)->InstrumentCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=False,require_federation=False)
