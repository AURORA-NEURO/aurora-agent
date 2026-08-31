"""Stewardship P32 local single-study contract_model snapshot-integrity feature F02."""
from .stewardship_snapshot_integrity_support import SnapshotIntegrityRequest4,SnapshotIntegrityCard7,SnapshotIntegrityError,manifest,qualify
FEATURE_ID="AFA-stewardship-P32-F02";CONTRACT_VERSION="stewardship-local-snapshot-integrity-contract_model/1.0"
def stewardship_local_snapshot_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
def qualify_stewardship_local_snapshot_integrity_contract_model(request:SnapshotIntegrityRequest4)->SnapshotIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
