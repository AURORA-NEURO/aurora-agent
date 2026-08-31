"""Worldgen P24 multimodal researcher/admin contract model surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F06"; CONTRACT_VERSION="worldgen-multimodal-researcher-admin-experience-contract_model/1.0"
def worldgen_multimodal_researcher_admin_experience_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")
def render_worldgen_multimodal_researcher_admin_experience_contract(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")

