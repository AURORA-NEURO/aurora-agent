"""Residue P32 local single-study workflow-fabric reconciliation-integrity feature F04."""
from .residue_reconciliation_integrity_support import ReconciliationIntegrityRequest4,ReconciliationIntegrityCard7,ReconciliationIntegrityError,manifest,qualify
FEATURE_ID="AFA-residue-P32-F04";CONTRACT_VERSION="residue-local_reconciliation_integrity_workflow_fabric/1.0"
def residue_local_reconciliation_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow-fabric")
def qualify_residue_local_reconciliation_integrity_workflow_fabric(request:ReconciliationIntegrityRequest4)->ReconciliationIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow-fabric")
