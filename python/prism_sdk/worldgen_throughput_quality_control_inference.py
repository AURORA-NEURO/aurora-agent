from .worldgen_quality_control_support import QualityControlRequest, QualityControlReceipt, assess
FEATURE_ID="AFA-worldgen-P07-F03"; CONTRACT_VERSION="worldgen-throughput-quality-control/1.0"
def worldgen_throughput_quality_control_inference_manifest(): return __import__("prism_sdk.worldgen_quality_control_support",fromlist=["manifest"]).manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityControlRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def assess_worldgen_throughput_quality_control(request:QualityControlRequest)->QualityControlReceipt: return assess(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=false)
