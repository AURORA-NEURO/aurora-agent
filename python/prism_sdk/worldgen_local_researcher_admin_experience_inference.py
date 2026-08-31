"""Worldgen P24 local researcher/admin inference surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F01"; CONTRACT_VERSION="worldgen-local-researcher-admin-experience-inference/1.0"
def worldgen_local_researcher_admin_experience_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def render_worldgen_local_researcher_admin_experience(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

