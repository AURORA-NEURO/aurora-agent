"""Stewardship P32 multimodal multi-study workflow_fabric snapshot-integrity feature F08."""
from .stewardship_snapshot_integrity_support import SnapshotIntegrityRequest4,SnapshotIntegrityCard7,SnapshotIntegrityError,manifest,qualify
FEATURE_ID="AFA-stewardship-P32-F08";CONTRACT_VERSION="stewardship-multimodal-snapshot-integrity-workflow_fabric/1.0"
def stewardship_multimodal_snapshot_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
def qualify_stewardship_multimodal_snapshot_integrity_workflow_fabric(request:SnapshotIntegrityRequest4)->SnapshotIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow_fabric")
