"""Worldgen P24 throughput researcher/admin contract model surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F07"; CONTRACT_VERSION="worldgen-throughput-researcher-admin-experience-contract_model/1.0"
def worldgen_throughput_researcher_admin_experience_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")
def render_worldgen_throughput_researcher_admin_experience_contract(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="contract model")

