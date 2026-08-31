"""World P32 local single-study research-copilot causal-integrity feature F03."""
from __future__ import annotations
from typing import Any,Mapping
from .world_causal_integrity_support import qualify,manifest
FEATURE_ID="AFA-world-P32-F03";CONTRACT_VERSION="world-local-causal-integrity-research-copilot/1.0"
def world_local_causal_integrity_research_copilot_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")
def qualify_world_local_causal_integrity_research_copilot(request:Mapping[str,Any])->dict[str,Any]:return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")

