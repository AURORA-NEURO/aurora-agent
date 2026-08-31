from .worldgen_publication_research_object_workflow_support import *
FEATURE_ID="AFA-worldgen-P16-F15"; CONTRACT_VERSION="worldgen-throughput-publication-research-object-workflow/1.0"
def worldgen_throughput_publication_research_object_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_publication_research_object_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

