"""Docgraph P32 local document graph integrity contract model."""
from .document_graph_integrity_support import *
FEATURE_ID="AFA-docgraph-P32-F05"; CONTRACT_VERSION="docgraph-local_document_graph_integrity_contract_model/1.0"
def local_document_graph_integrity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
def qualify_local_document_graph_integrity_contract_model(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract_model")
