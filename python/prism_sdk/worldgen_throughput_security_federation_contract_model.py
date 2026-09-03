from .worldgen_security_federation_contract_support import *
FEATURE_ID="AFA-worldgen-P20-F07"; CONTRACT_VERSION="worldgen-throughput-security-federation-contract/1.0"
def worldgen_throughput_security_federation_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def negotiate_worldgen_throughput_security_federation_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")

