"""Worldgen P04 AFA-worldgen-P04-F01 product wrapper."""
from .worldgen_knowledge_representation_support import KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt, manifest as _manifest, represent as _run
FEATURE_ID="AFA-worldgen-P04-F01"
CONTRACT_VERSION="worldgen-local-knowledge-representation/1.0"
def worldgen_local_knowledge_representation_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeRepresentationRequest1@1", scale="local single-study", autonomy_tier="A0")
def represent_worldgen_local_knowledge(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_federation=False)
__all__=["KnowledgeRepresentationRequest","KnowledgeRepresentationReceipt","worldgen_local_knowledge_representation_inference_manifest","represent_worldgen_local_knowledge"]
