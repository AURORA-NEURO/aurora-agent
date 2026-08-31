from .worldgen_security_federation_copilot_support import *
FEATURE_ID="AFA-worldgen-P20-F09"; CONTRACT_VERSION="worldgen-local-security-federation-copilot/1.0"
def worldgen_local_security_federation_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def run_worldgen_local_security_federation_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")

