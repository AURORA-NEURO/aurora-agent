"""Worldgen P04 AFA-worldgen-P04-F13 product wrapper."""
from .worldgen_knowledge_workflow_support import KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P04-F13"
CONTRACT_VERSION="worldgen-local-knowledge-workflow/1.0"
def worldgen_local_knowledge_representation_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeWorkflowRequest1@1", scale="local single-study", autonomy_tier="A1")
def schedule_worldgen_local_knowledge_representation_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_approval=False, require_federation=False)
__all__=["KnowledgeWorkflowRequest","KnowledgeWorkflowReceipt","worldgen_local_knowledge_representation_workflow_fabric_manifest","schedule_worldgen_local_knowledge_representation_workflow"]
