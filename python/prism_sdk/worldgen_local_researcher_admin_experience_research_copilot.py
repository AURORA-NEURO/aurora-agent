"""Worldgen P24 local researcher/admin research copilot surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F09"; CONTRACT_VERSION="worldgen-local-researcher-admin-experience-research_copilot/1.0"
def worldgen_local_researcher_admin_experience_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def render_worldgen_local_researcher_admin_experience_copilot(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")

