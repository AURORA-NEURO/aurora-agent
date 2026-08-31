"""Dataops P32 multimodal multi-study workflow-fabric ingestion-integrity feature F08."""
from .dataops_ingestion_integrity_support import IngestionIntegrityRequest4,IngestionIntegrityCard7,IngestionIntegrityError,manifest,qualify
FEATURE_ID="AFA-dataops-P32-F08";CONTRACT_VERSION="dataops-multimodal_ingestion_integrity_workflow_fabric/1.0"
def dataops_multimodal_ingestion_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow-fabric")
def qualify_dataops_multimodal_ingestion_integrity_workflow_fabric(request:IngestionIntegrityRequest4)->IngestionIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow-fabric")
