"""Stewardship P32 local single-study workflow_fabric snapshot-integrity feature F04."""
from .stewardship_snapshot_integrity_support import SnapshotIntegrityRequest4,SnapshotIntegrityCard7,SnapshotIntegrityError,manifest,qualify
FEATURE_ID="AFA-stewardship-P32-F04";CONTRACT_VERSION="stewardship-local-snapshot-integrity-workflow_fabric/1.0"
def stewardship_local_snapshot_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
def qualify_stewardship_local_snapshot_integrity_workflow_fabric(request:SnapshotIntegrityRequest4)->SnapshotIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow_fabric")
