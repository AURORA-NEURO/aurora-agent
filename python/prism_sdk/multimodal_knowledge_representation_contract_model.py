"""Worldgen P04 AFA-worldgen-P04-F06 product wrapper."""
from .worldgen_knowledge_contract_support import KnowledgeContractRequest, KnowledgeContractReceipt, manifest as _manifest, negotiate as _run
FEATURE_ID="AFA-worldgen-P04-F06"
CONTRACT_VERSION="worldgen-multimodal-knowledge-contract/1.0"
def worldgen_multimodal_knowledge_representation_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeContractRequest1@1", scale="multimodal multi-study", autonomy_tier="A0")
def negotiate_worldgen_multimodal_knowledge_representation_contract(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="multimodal multi-study", require_approval=False, require_federation=False)
__all__=["KnowledgeContractRequest","KnowledgeContractReceipt","worldgen_multimodal_knowledge_representation_contract_model_manifest","negotiate_worldgen_multimodal_knowledge_representation_contract"]
