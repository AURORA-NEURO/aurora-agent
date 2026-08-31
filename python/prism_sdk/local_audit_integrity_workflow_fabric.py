"""Sweep P32 local workflow_fabric audit-integrity feature F13."""
from .audit_integrity_support import AuditRequest4,AuditCard7,AuditIntegrityError,manifest,qualify
FEATURE_ID="AFA-sweep-P32-F13";CONTRACT_VERSION="sweep-local_audit_integrity_workflow_fabric/1.0"
def local_audit_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_audit_integrity_workflow_fabric(request:AuditRequest4)->AuditCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
