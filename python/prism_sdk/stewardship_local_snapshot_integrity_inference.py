"""Stewardship P32 local single-study inference snapshot-integrity feature F01."""
from .stewardship_snapshot_integrity_support import SnapshotIntegrityRequest4,SnapshotIntegrityCard7,SnapshotIntegrityError,manifest,qualify
FEATURE_ID="AFA-stewardship-P32-F01";CONTRACT_VERSION="stewardship-local-snapshot-integrity-inference/1.0"
def stewardship_local_snapshot_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def qualify_stewardship_local_snapshot_integrity_inference(request:SnapshotIntegrityRequest4)->SnapshotIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
