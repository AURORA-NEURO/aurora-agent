"""Build and exercise the distributable Python SDK without using the source tree at runtime."""

from __future__ import annotations

import configparser
from email.parser import BytesParser
from email.policy import default
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import venv
import zipfile


PYTHON_ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = PYTHON_ROOT / "prism_sdk"
PROJECT = tomllib.loads((PYTHON_ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]


def run(arguments: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        arguments,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {arguments!r}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def verify_wheel(wheel_path: Path) -> int:
    expected_modules = {
        path.relative_to(PYTHON_ROOT).as_posix()
        for path in PACKAGE_ROOT.rglob("*.py")
    }
    with zipfile.ZipFile(wheel_path) as wheel:
        names = set(wheel.namelist())
        missing = sorted(expected_modules - names)
        if missing:
            raise RuntimeError(f"wheel omits Python package modules: {missing[:12]!r}")

        metadata_paths = [name for name in names if name.endswith(".dist-info/METADATA")]
        if len(metadata_paths) != 1:
            raise RuntimeError("wheel must contain exactly one distribution METADATA file")
        metadata = BytesParser(policy=default).parsebytes(wheel.read(metadata_paths[0]))
        expected_name = PROJECT["name"].lower().replace("_", "-")
        actual_name = metadata.get("Name", "").lower().replace("_", "-")
        if actual_name != expected_name:
            raise RuntimeError("wheel distribution name does not match python/pyproject.toml")
        if metadata.get("Version") != PROJECT["version"]:
            raise RuntimeError("wheel version does not match python/pyproject.toml")
        if metadata.get("Requires-Python") != PROJECT["requires-python"]:
            raise RuntimeError("wheel Python requirement does not match python/pyproject.toml")
        if not PROJECT.get("dependencies", []) and metadata.get_all("Requires-Dist"):
            raise RuntimeError("dependency-free SDK wheel unexpectedly declares runtime dependencies")

        entrypoint_paths = [name for name in names if name.endswith(".dist-info/entry_points.txt")]
        if len(entrypoint_paths) != 1:
            raise RuntimeError("wheel must contain exactly one console-entry-point manifest")
        entrypoints = configparser.ConfigParser()
        entrypoints.read_string(wheel.read(entrypoint_paths[0]).decode("utf-8"))
        expected_entrypoint = PROJECT["scripts"]["aurora-agent"]
        actual_entrypoint = entrypoints.get("console_scripts", "aurora-agent", fallback=None)
        if actual_entrypoint != expected_entrypoint:
            raise RuntimeError("wheel CLI entry point does not match python/pyproject.toml")
    return len(expected_modules)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="aurora-python-wheel-") as temporary:
        temporary_root = Path(temporary)
        wheel_dir = temporary_root / "wheel"
        wheel_dir.mkdir()
        run(
            [
                sys.executable,
                "-m",
                "pip",
                "wheel",
                "--disable-pip-version-check",
                "--no-deps",
                "--wheel-dir",
                str(wheel_dir),
                str(PYTHON_ROOT),
            ],
            cwd=temporary_root,
        )
        wheels = list(wheel_dir.glob("*.whl"))
        if len(wheels) != 1:
            raise RuntimeError(f"expected one Python SDK wheel, found {len(wheels)}")
        module_count = verify_wheel(wheels[0])

        environment = temporary_root / "venv"
        venv.EnvBuilder(with_pip=True).create(environment)
        if os.name == "nt":
            interpreter = environment / "Scripts" / "python.exe"
            executable = environment / "Scripts" / "aurora-agent.exe"
        else:
            interpreter = environment / "bin" / "python"
            executable = environment / "bin" / "aurora-agent"
        run(
            [
                str(interpreter),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-deps",
                "--no-index",
                str(wheels[0]),
            ],
            cwd=temporary_root,
        )
        clean_env = os.environ.copy()
        clean_env.pop("PYTHONPATH", None)
        run(
            [
                str(interpreter),
                "-c",
                "import prism_sdk, prism_sdk.cli; from pathlib import Path; "
                "from prism_sdk.autonomy import AutonomousAgent; "
                "from prism_sdk.autonomous_goal_agent import AutonomousGoalAgentRuntime; "
                "from prism_sdk.autonomous_goal_control_loop import AutonomousGoalControlLoop; "
                "from prism_sdk.autonomous_goal_worker import AutonomousGoalWorker; "
                "assert callable(prism_sdk.cli.main); "
                "assert len(prism_sdk.__all__) == len(set(prism_sdk.__all__)), "
                "'public exports must be unique'; "
                "assert all(name in prism_sdk.__dict__ for name in prism_sdk.__all__), "
                "'every declared public export must resolve in the installed package'; "
                "autonomy_exports = ('AutonomousAgent', 'AutonomousGoalLedger', "
                "'AutonomousGoalControlLoop', 'AutonomousGoalWorker'); "
                "assert all(type(getattr(prism_sdk, name)).__name__ != '_UnavailableFeature' "
                "for name in autonomy_exports), 'core autonomy exports must be implemented'; "
                "assert all(callable(getattr(prism_sdk, name)) for name in autonomy_exports), "
                "'core autonomy exports must be callable'; "
                "async_exports = ((AutonomousGoalWorker, 'run_async'), "
                "(AutonomousGoalControlLoop, 'run_async'), "
                "(AutonomousGoalAgentRuntime, 'run_async'), "
                "(AutonomousGoalAgentRuntime, 'run_with_trace_async'), "
                "(AutonomousAgent, 'run_goal_control_loop_async')); "
                "assert all(callable(getattr(owner, method, None)) for owner, method in async_exports), "
                "'async autonomous goal entry points must be present in the installed wheel'; "
                "assert not hasattr(prism_sdk, 'aurora_sdk_symbol_that_does_not_exist'), "
                "'unknown exports must not be fabricated'; "
                "assert Path(prism_sdk.__file__).resolve().is_relative_to(Path.cwd() / 'venv')",
            ],
            cwd=temporary_root,
            env=clean_env,
        )
        help_output = run([str(executable), "--help"], cwd=temporary_root, env=clean_env)
        if "grounded-portfolio" not in help_output.stdout:
            raise RuntimeError("installed aurora-agent CLI help omits the grounded portfolio command")

    print(f"verified prism-sdk wheel: {module_count} modules installed; aurora-agent --help succeeded")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
