"""Worldgen P24 multimodal researcher/admin research copilot surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F10"; CONTRACT_VERSION="worldgen-multimodal-researcher-admin-experience-research_copilot/1.0"
def worldgen_multimodal_researcher_admin_experience_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def render_worldgen_multimodal_researcher_admin_experience_copilot(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")

