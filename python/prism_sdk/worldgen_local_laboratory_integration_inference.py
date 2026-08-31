from .worldgen_laboratory_integration_support import InstrumentActionRequest, InstrumentActionReceipt, integrate, manifest
FEATURE_ID="AFA-worldgen-P11-F01"; CONTRACT_VERSION="worldgen-local-laboratory_integration/1.0"
def worldgen_local_laboratory_integration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentActionRequest1@1",scale="local single-study",autonomy_tier="A0")
def integrate_worldgen_local_laboratory_integrations(request:InstrumentActionRequest)->InstrumentActionReceipt: return integrate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
