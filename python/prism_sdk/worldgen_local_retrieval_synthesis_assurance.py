"""Local retrieval-synthesis assurance harness (AFA-worldgen-P02-F25)."""
from .worldgen_retrieval_assurance_support import RetrievalAssuranceRequest, RetrievalAssuranceReceipt, assure, manifest
FEATURE_ID="AFA-worldgen-P02-F25"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-assurance/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"
def worldgen_local_retrieval_synthesis_assurance_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="local single-study",autonomy_tier="A0")
def assure_worldgen_local_retrieval_synthesis(request:RetrievalAssuranceRequest)->RetrievalAssuranceReceipt:return assure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["RetrievalAssuranceRequest","RetrievalAssuranceReceipt","assure_worldgen_local_retrieval_synthesis","worldgen_local_retrieval_synthesis_assurance_manifest"]
