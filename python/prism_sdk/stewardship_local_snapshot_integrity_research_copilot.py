"""Stewardship P32 local single-study research_copilot snapshot-integrity feature F03."""
from .stewardship_snapshot_integrity_support import SnapshotIntegrityRequest4,SnapshotIntegrityCard7,SnapshotIntegrityError,manifest,qualify
FEATURE_ID="AFA-stewardship-P32-F03";CONTRACT_VERSION="stewardship-local-snapshot-integrity-research_copilot/1.0"
def stewardship_local_snapshot_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
def qualify_stewardship_local_snapshot_integrity_research_copilot(request:SnapshotIntegrityRequest4)->SnapshotIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
