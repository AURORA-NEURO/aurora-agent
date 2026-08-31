"""Governance P32 local single-study contract_model evolution-integrity feature F02."""
from .governance_evolution_integrity_support import EvolutionIntegrityRequest4,EvolutionIntegrityCard7,EvolutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-governance-P32-F02"; CONTRACT_VERSION="governance-local-evolution-integrity-contract_model/1.0"
def governance_local_evolution_integrity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
def qualify_governance_local_evolution_integrity_contract_model(request:EvolutionIntegrityRequest4)->EvolutionIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
