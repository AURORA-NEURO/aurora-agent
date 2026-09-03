"""Worldgen P04 AFA-worldgen-P04-F12 product wrapper."""
from .worldgen_knowledge_copilot_support import KnowledgeCopilotRequest, KnowledgeCopilotReceipt, manifest as _manifest, run as _run
FEATURE_ID="AFA-worldgen-P04-F12"
CONTRACT_VERSION="worldgen-federated-continual-knowledge-copilot/1.0"
def worldgen_federated_continual_knowledge_representation_research_copilot_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeCopilotRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def run_worldgen_federated_continual_knowledge_representation_copilot(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_approval=True, require_federation=True)
__all__=["KnowledgeCopilotRequest","KnowledgeCopilotReceipt","worldgen_federated_continual_knowledge_representation_research_copilot_manifest","run_worldgen_federated_continual_knowledge_representation_copilot"]
