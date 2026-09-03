"""Ops P32 local single-study workflow_fabric run-integrity feature F04."""
from .ops_run_integrity_support import RunIntegrityRequest4,RunIntegrityCard7,RunIntegrityError,manifest,qualify
FEATURE_ID="AFA-ops-P32-F04";CONTRACT_VERSION="ops-local-run-integrity-workflow_fabric/1.0"
def ops_local_run_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
def qualify_ops_local_run_integrity_workflow_fabric(request:RunIntegrityRequest4)->RunIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
