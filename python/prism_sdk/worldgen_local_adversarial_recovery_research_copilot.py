"""Worldgen P30 local single-study research copilot surface (F09)."""
from .worldgen_adversarial_recovery_support import *
FEATURE_ID="AFA-worldgen-P30-F09"; CONTRACT_VERSION="worldgen-local-adversarial-recovery-research_copilot/1.0"
def worldgen_local_adversarial_recovery_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def recover_worldgen_local_adversarial_recovery_copilot(request): return recover(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")

