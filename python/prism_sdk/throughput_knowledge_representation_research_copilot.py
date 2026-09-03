"""Worldgen P04 AFA-worldgen-P04-F11 product wrapper."""
from .worldgen_knowledge_copilot_support import KnowledgeCopilotRequest, KnowledgeCopilotReceipt, manifest as _manifest, run as _run
FEATURE_ID="AFA-worldgen-P04-F11"
CONTRACT_VERSION="worldgen-throughput-knowledge-copilot/1.0"
def worldgen_throughput_knowledge_representation_research_copilot_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeCopilotRequest1@1", scale="prospective high-throughput", autonomy_tier="A2")
def run_worldgen_throughput_knowledge_representation_copilot(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_approval=True, require_federation=False)
__all__=["KnowledgeCopilotRequest","KnowledgeCopilotReceipt","worldgen_throughput_knowledge_representation_research_copilot_manifest","run_worldgen_throughput_knowledge_representation_copilot"]
