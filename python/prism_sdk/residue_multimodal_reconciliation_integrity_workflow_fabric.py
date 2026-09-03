"""Residue P32 multimodal multi-study workflow-fabric reconciliation-integrity feature F08."""
from .residue_reconciliation_integrity_support import ReconciliationIntegrityRequest4,ReconciliationIntegrityCard7,ReconciliationIntegrityError,manifest,qualify
FEATURE_ID="AFA-residue-P32-F08";CONTRACT_VERSION="residue-multimodal_reconciliation_integrity_workflow_fabric/1.0"
def residue_multimodal_reconciliation_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow-fabric")
def qualify_residue_multimodal_reconciliation_integrity_workflow_fabric(request:ReconciliationIntegrityRequest4)->ReconciliationIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow-fabric")
