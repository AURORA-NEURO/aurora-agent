"""Worldgen P31 multimodal multi-study research copilot surface (F10)."""
from .worldgen_federated_commons_support import *
FEATURE_ID="AFA-worldgen-P31-F10"; CONTRACT_VERSION="worldgen-multimodal-federated-commons-research_copilot/1.0"
def worldgen_multimodal_federated_commons_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def admit_worldgen_multimodal_federated_commons_copilot(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
