"""Docgraph P32 local document graph integrity feature."""
from .document_graph_integrity_support import *
FEATURE_ID="AFA-docgraph-P32-F01"; CONTRACT_VERSION="docgraph-local_document_graph_integrity_inference/1.0"
def local_document_graph_integrity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def qualify_local_document_graph_integrity_inference(request): return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
