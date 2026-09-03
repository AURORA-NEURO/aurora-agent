"""Worldgen P22 multimodal interoperability/extensibility workflow-fabric surface."""
from .worldgen_interoperability_extensibility_support import *
FEATURE_ID="AFA-worldgen-P22-F14"; CONTRACT_VERSION="worldgen-multimodal-interoperability-extensibility-workflow/1.0"
def worldgen_multimodal_interoperability_extensibility_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow")
def schedule_worldgen_multimodal_interoperability_extensibility_workflow(request): return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow")
