"""Worldgen P30 local single-study contract model surface (F05)."""
from .worldgen_adversarial_recovery_support import *
FEATURE_ID="AFA-worldgen-P30-F05"; CONTRACT_VERSION="worldgen-local-adversarial-recovery-contract_model/1.0"
def worldgen_local_adversarial_recovery_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def recover_worldgen_local_adversarial_recovery_contract(request): return recover(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

