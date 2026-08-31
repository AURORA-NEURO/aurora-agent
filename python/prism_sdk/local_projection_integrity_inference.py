"""Graph P32 local inference projection-integrity feature F01."""
from .projection_integrity_support import ProjectionRequest4,ProjectionCard7,ProjectionIntegrityError,manifest,qualify
FEATURE_ID="AFA-graph-P32-F01";CONTRACT_VERSION="graph-local_projection_integrity_inference/1.0"
def local_projection_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def qualify_local_projection_integrity_inference(request:ProjectionRequest4)->ProjectionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
