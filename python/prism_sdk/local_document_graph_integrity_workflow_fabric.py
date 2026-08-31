"""Docgraph P32 local document graph integrity workflow fabric."""
from .document_graph_integrity_support import *
FEATURE_ID="AFA-docgraph-P32-F13"; CONTRACT_VERSION="docgraph-local_document_graph_integrity_workflow_fabric/1.0"
def local_document_graph_integrity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_document_graph_integrity_workflow_fabric(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
