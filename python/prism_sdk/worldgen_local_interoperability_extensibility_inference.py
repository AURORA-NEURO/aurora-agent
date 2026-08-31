"""Worldgen P22 local interoperability/extensibility inference surface."""
from .worldgen_interoperability_extensibility_support import *
FEATURE_ID="AFA-worldgen-P22-F01"; CONTRACT_VERSION="worldgen-local-interoperability-extensibility/1.0"
def worldgen_local_interoperability_extensibility_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def negotiate_worldgen_local_interoperability_extensibility(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
