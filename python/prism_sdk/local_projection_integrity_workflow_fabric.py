"""Graph P32 local workflow_fabric projection-integrity feature F13."""
from .projection_integrity_support import ProjectionRequest4,ProjectionCard7,ProjectionIntegrityError,manifest,qualify
FEATURE_ID="AFA-graph-P32-F13";CONTRACT_VERSION="graph-local_projection_integrity_workflow_fabric/1.0"
def local_projection_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def qualify_local_projection_integrity_workflow_fabric(request:ProjectionRequest4)->ProjectionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
