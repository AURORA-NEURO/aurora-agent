"""Worldgen P31 local single-study inference surface (F01)."""
from .worldgen_federated_commons_support import *
FEATURE_ID="AFA-worldgen-P31-F01"; CONTRACT_VERSION="worldgen-local-federated-commons-inference/1.0"
def worldgen_local_federated_commons_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def admit_worldgen_local_federated_commons(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
