from .worldgen_publication_research_object_copilot_support import *
FEATURE_ID="AFA-worldgen-P16-F11"; CONTRACT_VERSION="worldgen-throughput-publication-research-object-copilot/1.0"
def worldgen_throughput_publication_research_object_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def run_worldgen_throughput_publication_research_object_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

