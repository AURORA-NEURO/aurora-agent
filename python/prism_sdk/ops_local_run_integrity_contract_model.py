"""Ops P32 local single-study contract_model run-integrity feature F02."""
from .ops_run_integrity_support import RunIntegrityRequest4,RunIntegrityCard7,RunIntegrityError,manifest,qualify
FEATURE_ID="AFA-ops-P32-F02";CONTRACT_VERSION="ops-local-run-integrity-contract_model/1.0"
def ops_local_run_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
def qualify_ops_local_run_integrity_contract_model(request:RunIntegrityRequest4)->RunIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract_model")
