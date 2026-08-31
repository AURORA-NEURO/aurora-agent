from .worldgen_quality_control_support import QualityControlRequest, QualityControlReceipt, assess
FEATURE_ID="AFA-worldgen-P07-F04"; CONTRACT_VERSION="worldgen-federated_continual-quality-control/1.0"
def worldgen_federated_continual_quality_control_inference_manifest(): return __import__("prism_sdk.worldgen_quality_control_support",fromlist=["manifest"]).manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityControlRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def assess_worldgen_federated_continual_quality_control(request:QualityControlRequest)->QualityControlReceipt: return assess(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=true)
