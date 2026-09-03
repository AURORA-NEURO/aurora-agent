from .worldgen_quality_control_support import QualityControlRequest, QualityControlReceipt, assess
FEATURE_ID="AFA-worldgen-P07-F01"; CONTRACT_VERSION="worldgen-local-quality-control/1.0"
def worldgen_local_quality_control_inference_manifest(): return __import__("prism_sdk.worldgen_quality_control_support",fromlist=["manifest"]).manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityControlRequest1@1",scale="local single-study",autonomy_tier="A0")
def assess_worldgen_local_quality_control(request:QualityControlRequest)->QualityControlReceipt: return assess(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=false)
