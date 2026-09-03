"""Worldgen P24 local researcher/admin workflow fabric surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F13"; CONTRACT_VERSION="worldgen-local-researcher-admin-experience-workflow_fabric/1.0"
def worldgen_local_researcher_admin_experience_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def render_worldgen_local_researcher_admin_experience_workflow(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

