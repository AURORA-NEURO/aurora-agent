"""Worldgen P04 AFA-worldgen-P04-F15 product wrapper."""
from .worldgen_knowledge_workflow_support import KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P04-F15"
CONTRACT_VERSION="worldgen-throughput-knowledge-workflow/1.0"
def worldgen_throughput_knowledge_representation_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeWorkflowRequest1@1", scale="prospective high-throughput", autonomy_tier="A2")
def schedule_worldgen_throughput_knowledge_representation_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_approval=True, require_federation=False)
__all__=["KnowledgeWorkflowRequest","KnowledgeWorkflowReceipt","worldgen_throughput_knowledge_representation_workflow_fabric_manifest","schedule_worldgen_throughput_knowledge_representation_workflow"]
