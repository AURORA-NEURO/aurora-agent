"""Residue P32 local single-study contract-model reconciliation-integrity feature F02."""
from .residue_reconciliation_integrity_support import ReconciliationIntegrityRequest4,ReconciliationIntegrityCard7,ReconciliationIntegrityError,manifest,qualify
FEATURE_ID="AFA-residue-P32-F02";CONTRACT_VERSION="residue-local_reconciliation_integrity_contract_model/1.0"
def residue_local_reconciliation_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
def qualify_residue_local_reconciliation_integrity_contract_model(request:ReconciliationIntegrityRequest4)->ReconciliationIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
