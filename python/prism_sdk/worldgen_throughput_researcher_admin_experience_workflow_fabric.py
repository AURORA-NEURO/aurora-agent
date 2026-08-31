"""Worldgen P24 throughput researcher/admin workflow fabric surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F15"; CONTRACT_VERSION="worldgen-throughput-researcher-admin-experience-workflow_fabric/1.0"
def worldgen_throughput_researcher_admin_experience_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")
def render_worldgen_throughput_researcher_admin_experience_workflow(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")

