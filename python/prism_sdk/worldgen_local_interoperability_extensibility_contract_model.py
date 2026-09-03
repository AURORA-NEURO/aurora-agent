"""Worldgen P22 local interoperability/extensibility contract-model surface."""
from .worldgen_interoperability_extensibility_support import *
FEATURE_ID="AFA-worldgen-P22-F05"; CONTRACT_VERSION="worldgen-local-interoperability-extensibility-contract/1.0"
def worldgen_local_interoperability_extensibility_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract")
def negotiate_worldgen_local_interoperability_extensibility_contract(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract")
