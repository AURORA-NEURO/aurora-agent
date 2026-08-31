"""Worldgen P04 AFA-worldgen-P04-F08 product wrapper."""
from .worldgen_knowledge_contract_support import KnowledgeContractRequest, KnowledgeContractReceipt, manifest as _manifest, negotiate as _run
FEATURE_ID="AFA-worldgen-P04-F08"
CONTRACT_VERSION="worldgen-federated-continual-knowledge-contract/1.0"
def worldgen_federated_continual_knowledge_representation_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="KnowledgeContractRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def negotiate_worldgen_federated_continual_knowledge_representation_contract(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_approval=False, require_federation=True)
__all__=["KnowledgeContractRequest","KnowledgeContractReceipt","worldgen_federated_continual_knowledge_representation_contract_model_manifest","negotiate_worldgen_federated_continual_knowledge_representation_contract"]
