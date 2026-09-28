from __future__ import annotations

import importlib
import inspect
import unittest
from pathlib import Path
from unittest.mock import patch


SDK_PACKAGE = Path(__file__).resolve().parents[1] / "prism_sdk"
FEATURE_FAMILIES = ("experiment_design", "mechanism_exploration", "quality_control")
SURFACES = {"contract_model", "inference", "research_copilot", "workflow_fabric"}
SCALE_FLAGS = {
    "federated_continual": {"require_approval": False, "require_federation": True},
    "local": {"require_approval": True, "require_federation": False},
    "multimodal": {"require_approval": False, "require_federation": False},
    "throughput": {"require_approval": False, "require_federation": False},
}


def generated_wrapper_modules() -> list[str]:
    modules = []
    for path in SDK_PACKAGE.glob("worldgen_*.py"):
        stem = path.stem
        if not any(stem.endswith(f"_{surface}") for surface in SURFACES):
            continue
        if not any(f"_{family}_" in stem for family in FEATURE_FAMILIES):
            continue
        if any(stem.startswith(f"worldgen_{scale}_") for scale in SCALE_FLAGS):
            modules.append(stem)
    return sorted(modules)


class WorldgenWrapperBooleanFlagsTests(unittest.TestCase):
    def test_every_scale_wrapper_forwards_python_boolean_policy(self) -> None:
        modules = generated_wrapper_modules()
        self.assertGreaterEqual(len(modules), 37, "the wrapper walk must cover all current scale variants")

        for module_name in modules:
            with self.subTest(module=module_name):
                module = importlib.import_module(f"prism_sdk.{module_name}")
                wrappers = [
                    value
                    for name, value in vars(module).items()
                    if inspect.isfunction(value)
                    and value.__module__ == module.__name__
                    and not name.endswith("_manifest")
                ]
                self.assertEqual(len(wrappers), 1, "each generated surface must expose one entrypoint")
                wrapper = wrappers[0]

                delegates = [
                    name
                    for name in wrapper.__code__.co_names
                    if name in vars(module) and callable(vars(module)[name])
                ]
                self.assertEqual(len(delegates), 1, "the wrapper must delegate through one support function")

                request = object()
                receipt = object()
                with patch.object(module, delegates[0], return_value=receipt) as delegate:
                    self.assertIs(wrapper(request), receipt)

                self.assertEqual(delegate.call_args.args, (request,))
                flags = delegate.call_args.kwargs
                self.assertIn("require_federation", flags)
                expected = SCALE_FLAGS[next(scale for scale in SCALE_FLAGS if module_name.startswith(f"worldgen_{scale}_"))]
                for flag, expected_value in expected.items():
                    if flag in flags:
                        self.assertIs(type(flags[flag]), bool, f"{flag} must be a Python bool")
                        self.assertIs(flags[flag], expected_value, f"{flag} must follow the scale policy")


if __name__ == "__main__":
    unittest.main()
