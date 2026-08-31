"""Bioethics P32 local workflow-fabric boundary-integrity feature F04."""
from .bioethics_boundary_integrity_support import BoundaryIntegrityRequest4,BoundaryIntegrityCard7,BoundaryIntegrityError,manifest,qualify
FEATURE_ID="AFA-bioethics-P32-F04";CONTRACT_VERSION="bioethics-local_boundary_integrity_workflow_fabric/1.0"
def bioethics_local_boundary_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
def qualify_bioethics_local_boundary_integrity_workflow_fabric(request:BoundaryIntegrityRequest4)->BoundaryIntegrityCard7:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow-fabric")
