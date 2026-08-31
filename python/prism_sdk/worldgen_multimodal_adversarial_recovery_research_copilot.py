"""Worldgen P30 multimodal multi-study research copilot surface (F10)."""
from .worldgen_adversarial_recovery_support import *
FEATURE_ID="AFA-worldgen-P30-F10"; CONTRACT_VERSION="worldgen-multimodal-adversarial-recovery-research_copilot/1.0"
def worldgen_multimodal_adversarial_recovery_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def recover_worldgen_multimodal_adversarial_recovery_copilot(request): return recover(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")

