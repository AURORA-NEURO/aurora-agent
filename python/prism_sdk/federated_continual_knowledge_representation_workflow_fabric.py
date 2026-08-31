"""Worldgen P04 AFA-worldgen-P04-F16 product wrapper."""
from .worldgen_knowledge_workflow_support import KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P04-F16"
CONTRACT_VERSION="worldgen-federated-continual-knowledge-workflow/1.0"
def worldgen_federated_continual_knowledge_representation_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeWorkflowRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def schedule_worldgen_federated_continual_knowledge_representation_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_approval=True, require_federation=True)
__all__=["KnowledgeWorkflowRequest","KnowledgeWorkflowReceipt","worldgen_federated_continual_knowledge_representation_workflow_fabric_manifest","schedule_worldgen_federated_continual_knowledge_representation_workflow"]
