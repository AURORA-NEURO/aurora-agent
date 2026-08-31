"""Ops P32 local single-study research_copilot run-integrity feature F03."""
from .ops_run_integrity_support import RunIntegrityRequest4,RunIntegrityCard7,RunIntegrityError,manifest,qualify
FEATURE_ID="AFA-ops-P32-F03";CONTRACT_VERSION="ops-local-run-integrity-research_copilot/1.0"
def ops_local_run_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
def qualify_ops_local_run_integrity_research_copilot(request:RunIntegrityRequest4)->RunIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research_copilot")
