"""Infra P32 local contract-model reliability-integrity feature F02."""
from .infra_reliability_integrity_support import ReliabilityIntegrityRequest4,ReliabilityIntegrityCard7,ReliabilityIntegrityError,manifest,qualify
FEATURE_ID="AFA-infra-P32-F02";CONTRACT_VERSION="infra-local_reliability_integrity_contract_model/1.0"
def infra_local_reliability_integrity_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract-model")
def qualify_infra_local_reliability_integrity_contract_model(request:ReliabilityIntegrityRequest4)->ReliabilityIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="contract-model")
