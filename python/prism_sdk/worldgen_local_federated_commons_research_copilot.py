"""Worldgen P31 local single-study research copilot surface (F09)."""
from .worldgen_federated_commons_support import *
FEATURE_ID="AFA-worldgen-P31-F09"; CONTRACT_VERSION="worldgen-local-federated-commons-research_copilot/1.0"
def worldgen_local_federated_commons_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def admit_worldgen_local_federated_commons_copilot(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
