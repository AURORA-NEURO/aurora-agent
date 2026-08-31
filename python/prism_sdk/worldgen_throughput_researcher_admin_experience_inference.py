"""Worldgen P24 throughput researcher/admin inference surface."""
from .worldgen_researcher_admin_experience_support import *
FEATURE_ID="AFA-worldgen-P24-F03"; CONTRACT_VERSION="worldgen-throughput-researcher-admin-experience-inference/1.0"
def worldgen_throughput_researcher_admin_experience_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")
def render_worldgen_throughput_researcher_admin_experience(request): return render(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="inference")

