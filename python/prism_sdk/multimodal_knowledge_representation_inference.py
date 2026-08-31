"""Worldgen P04 AFA-worldgen-P04-F02 product wrapper."""
from .worldgen_knowledge_representation_support import KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt, manifest as _manifest, represent as _run
FEATURE_ID="AFA-worldgen-P04-F02"
CONTRACT_VERSION="worldgen-multimodal-knowledge-representation/1.0"
def worldgen_multimodal_knowledge_representation_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeRepresentationRequest1@1", scale="multimodal multi-study", autonomy_tier="A1")
def represent_worldgen_multimodal_knowledge(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="multimodal multi-study", require_federation=False)
__all__=["KnowledgeRepresentationRequest","KnowledgeRepresentationReceipt","worldgen_multimodal_knowledge_representation_inference_manifest","represent_worldgen_multimodal_knowledge"]
