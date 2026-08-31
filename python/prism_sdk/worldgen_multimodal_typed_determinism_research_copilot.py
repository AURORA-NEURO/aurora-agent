from .worldgen_typed_determinism_copilot_support import *
FEATURE_ID="AFA-worldgen-P17-F10"; CONTRACT_VERSION="worldgen-multimodal-typed-determinism-copilot/1.0"
def worldgen_multimodal_typed_determinism_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def run_worldgen_multimodal_typed_determinism_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

