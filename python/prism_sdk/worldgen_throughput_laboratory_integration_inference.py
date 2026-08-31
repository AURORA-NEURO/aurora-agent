from .worldgen_laboratory_integration_support import InstrumentActionRequest, InstrumentActionReceipt, integrate, manifest
FEATURE_ID="AFA-worldgen-P11-F03"; CONTRACT_VERSION="worldgen-throughput-laboratory_integration/1.0"
def worldgen_throughput_laboratory_integration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentActionRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def integrate_worldgen_throughput_laboratory_integrations(request:InstrumentActionRequest)->InstrumentActionReceipt: return integrate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=False)
