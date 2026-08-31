"""Worldgen P04 AFA-worldgen-P04-F04 product wrapper."""
from .worldgen_knowledge_representation_support import KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt, manifest as _manifest, represent as _run
FEATURE_ID="AFA-worldgen-P04-F04"
CONTRACT_VERSION="worldgen-federated-continual-knowledge-representation/1.0"
def worldgen_federated_continual_knowledge_representation_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeRepresentationRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def represent_worldgen_federated_continual_knowledge(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_federation=True)
__all__=["KnowledgeRepresentationRequest","KnowledgeRepresentationReceipt","worldgen_federated_continual_knowledge_representation_inference_manifest","represent_worldgen_federated_continual_knowledge"]
