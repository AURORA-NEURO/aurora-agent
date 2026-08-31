"""Infra P32 local workflow-fabric reliability-integrity feature F04."""
from .infra_reliability_integrity_support import ReliabilityIntegrityRequest4,ReliabilityIntegrityCard7,ReliabilityIntegrityError,manifest,qualify
FEATURE_ID="AFA-infra-P32-F04";CONTRACT_VERSION="infra-local_reliability_integrity_workflow_fabric/1.0"
def infra_local_reliability_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
def qualify_infra_local_reliability_integrity_workflow_fabric(request:ReliabilityIntegrityRequest4)->ReliabilityIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
