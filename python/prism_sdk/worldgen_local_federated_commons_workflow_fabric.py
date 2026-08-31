"""Worldgen P31 local single-study workflow fabric surface (F13)."""
from .worldgen_federated_commons_support import *
FEATURE_ID="AFA-worldgen-P31-F13"; CONTRACT_VERSION="worldgen-local-federated-commons-workflow_fabric/1.0"
def worldgen_local_federated_commons_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def admit_worldgen_local_federated_commons_workflow(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
