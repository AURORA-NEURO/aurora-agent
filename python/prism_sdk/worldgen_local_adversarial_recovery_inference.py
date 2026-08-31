"""Worldgen P30 local single-study inference surface (F01)."""
from .worldgen_adversarial_recovery_support import *
FEATURE_ID="AFA-worldgen-P30-F01"; CONTRACT_VERSION="worldgen-local-adversarial-recovery-inference/1.0"
def worldgen_local_adversarial_recovery_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def recover_worldgen_local_adversarial_recovery(request): return recover(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

