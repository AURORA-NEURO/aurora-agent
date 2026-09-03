"""Federated continual retrieval-synthesis assurance harness (AFA-worldgen-P02-F28)."""
from .worldgen_retrieval_assurance_support import RetrievalAssuranceRequest, RetrievalAssuranceReceipt, assure, manifest
FEATURE_ID="AFA-worldgen-P02-F28"; CONTRACT_VERSION="worldgen-federated-continual-retrieval-synthesis-assurance/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery4@1"
def worldgen_federated_continual_retrieval_synthesis_assurance_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual",autonomy_tier="A2")
def assure_worldgen_federated_continual_retrieval_synthesis(request:RetrievalAssuranceRequest)->RetrievalAssuranceReceipt:return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["RetrievalAssuranceRequest","RetrievalAssuranceReceipt","assure_worldgen_federated_continual_retrieval_synthesis","worldgen_federated_continual_retrieval_synthesis_assurance_manifest"]
