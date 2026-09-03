"""Worldgen P22 local interoperability/extensibility research-copilot surface."""
from .worldgen_interoperability_extensibility_support import *
FEATURE_ID="AFA-worldgen-P22-F09"; CONTRACT_VERSION="worldgen-local-interoperability-extensibility-copilot/1.0"
def worldgen_local_interoperability_extensibility_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="copilot")
def run_worldgen_local_interoperability_extensibility_research_copilot(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="copilot")
