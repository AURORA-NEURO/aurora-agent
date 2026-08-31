"""Governance P32 local single-study research_copilot evolution-integrity feature F03."""
from .governance_evolution_integrity_support import EvolutionIntegrityRequest4,EvolutionIntegrityCard7,EvolutionIntegrityError,manifest,qualify
FEATURE_ID="AFA-governance-P32-F03"; CONTRACT_VERSION="governance-local-evolution-integrity-research_copilot/1.0"
def governance_local_evolution_integrity_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
def qualify_governance_local_evolution_integrity_research_copilot(request:EvolutionIntegrityRequest4)->EvolutionIntegrityCard7: return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
