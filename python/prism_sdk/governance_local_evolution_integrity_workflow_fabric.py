"""Governance P32 local single-study workflow_fabric evolution-integrity feature F04."""
from .governance_evolution_integrity_support import EvolutionIntegrityRequest4,EvolutionIntegrityCard7,EvolutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-governance-P32-F04"; CONTRACT_VERSION="governance-local-evolution-integrity-workflow_fabric/1.0"
def governance_local_evolution_integrity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
def qualify_governance_local_evolution_integrity_workflow_fabric(request:EvolutionIntegrityRequest4)->EvolutionIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
