"""Worldgen P31 local single-study contract model surface (F05)."""
from .worldgen_federated_commons_support import *
FEATURE_ID="AFA-worldgen-P31-F05"; CONTRACT_VERSION="worldgen-local-federated-commons-contract_model/1.0"
def worldgen_local_federated_commons_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def admit_worldgen_local_federated_commons_contract(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
