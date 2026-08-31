"""Section P32 local single-study research-copilot closure-integrity feature F03."""
from __future__ import annotations
from typing import Any,Mapping
from .section_closure_integrity_support import compile_closure,manifest
FEATURE_ID="AFA-section-P32-F03";CONTRACT_VERSION="section-local-closure-integrity-research-copilot/1.0"
def compile_section_local_closure_integrity_research_copilot_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")
def compile_section_local_closure_integrity_research_copilot(request:Mapping[str,Any])->dict[str,Any]:return compile_closure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")
