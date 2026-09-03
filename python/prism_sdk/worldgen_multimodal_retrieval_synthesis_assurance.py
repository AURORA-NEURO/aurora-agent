"""Multimodal retrieval-synthesis assurance harness (AFA-worldgen-P02-F26)."""
from .worldgen_retrieval_assurance_support import RetrievalAssuranceRequest, RetrievalAssuranceReceipt, assure, manifest
FEATURE_ID="AFA-worldgen-P02-F26"; CONTRACT_VERSION="worldgen-multimodal-retrieval-synthesis-assurance/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery2@1"
def worldgen_multimodal_retrieval_synthesis_assurance_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def assure_worldgen_multimodal_retrieval_synthesis(request:RetrievalAssuranceRequest)->RetrievalAssuranceReceipt:return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["RetrievalAssuranceRequest","RetrievalAssuranceReceipt","assure_worldgen_multimodal_retrieval_synthesis","worldgen_multimodal_retrieval_synthesis_assurance_manifest"]
