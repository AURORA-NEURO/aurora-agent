"""Worldgen P30 local single-study workflow fabric surface (F13)."""
from .worldgen_adversarial_recovery_support import *
FEATURE_ID="AFA-worldgen-P30-F13"; CONTRACT_VERSION="worldgen-local-adversarial-recovery-workflow_fabric/1.0"
def worldgen_local_adversarial_recovery_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def recover_worldgen_local_adversarial_recovery_workflow(request): return recover(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

