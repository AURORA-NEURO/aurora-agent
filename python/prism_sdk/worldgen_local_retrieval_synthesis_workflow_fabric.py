"""AFA-worldgen-P02-F13 local retrieval-synthesis workflow fabric."""
from .worldgen_retrieval_workflow_support import RetrievalWorkflowRequest, RetrievalWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P02-F13"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-workflow/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"; OUTPUT_SCHEMA="EvidenceSynthesis4@1"; SCALE="local single-study"
def worldgen_local_retrieval_synthesis_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A1")
def schedule_worldgen_local_retrieval_synthesis_workflow(request): return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","RetrievalWorkflowRequest","RetrievalWorkflowReceipt","worldgen_local_retrieval_synthesis_workflow_fabric_manifest","schedule_worldgen_local_retrieval_synthesis_workflow"]
