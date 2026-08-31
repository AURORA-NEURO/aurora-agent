"""Dataops P32 local single-study contract-model ingestion-integrity feature F02."""
from .dataops_ingestion_integrity_support import IngestionIntegrityRequest4,IngestionIntegrityCard7,IngestionIntegrityError,manifest,qualify
FEATURE_ID="AFA-dataops-P32-F02";CONTRACT_VERSION="dataops-local_ingestion_integrity_contract_model/1.0"
def dataops_local_ingestion_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
def qualify_dataops_local_ingestion_integrity_contract_model(request:IngestionIntegrityRequest4)->IngestionIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
