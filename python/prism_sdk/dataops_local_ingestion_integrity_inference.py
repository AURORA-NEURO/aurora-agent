"""Dataops P32 local single-study inference ingestion-integrity feature F01."""
from .dataops_ingestion_integrity_support import IngestionIntegrityRequest4,IngestionIntegrityCard7,IngestionIntegrityError,manifest,qualify
FEATURE_ID="AFA-dataops-P32-F01";CONTRACT_VERSION="dataops-local_ingestion_integrity_inference/1.0"
def dataops_local_ingestion_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_dataops_local_ingestion_integrity_inference(request:IngestionIntegrityRequest4)->IngestionIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
