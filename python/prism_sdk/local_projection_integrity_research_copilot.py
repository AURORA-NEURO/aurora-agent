"""Graph P32 local research_copilot projection-integrity feature F09."""
from .projection_integrity_support import ProjectionRequest4,ProjectionCard7,ProjectionIntegrityError,manifest,qualify
FEATURE_ID="AFA-graph-P32-F09";CONTRACT_VERSION="graph-local_projection_integrity_research_copilot/1.0"
def local_projection_integrity_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
def qualify_local_projection_integrity_research_copilot(request:ProjectionRequest4)->ProjectionCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="research_copilot")
