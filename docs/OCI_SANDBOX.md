# Docker command boundary

`bioprism_runtime::DockerSandbox` is an opt-in command runner for a local Docker Engine. It is a
separate API from `ExecutorProvider`: the SDK plugin dispatcher, `SubprocessProvider`,
`ContainerProvider`, workflow runner, and trial lifecycle do not invoke it. A sandbox declaration
or a successful simulation therefore does not show that this boundary was used.

`DockerProcessSource` adapts `ProcessSpawn` effects to this runner for callers that explicitly
construct a `RecordingHost`. The existing effect policy authorizes each request before the source
is called, and the resulting bounded response is recorded on the `WorldTape` for deterministic
replay. Configure one source per run with a fixed digest-pinned image, read-only input directory,
resource limits, and a quarantine root. Each invocation writes to a new private per-run directory.
The tape and workload receive only the quarantine identifier and artifact metadata; the trusted host
can use `DockerProcessSource::quarantined_outputs` to locate the retained files. All outputs remain
in quarantine because this crate does not implement artifact scanning, independent review, or
release. Other effect kinds are delegated to the source's configured fallback, commonly
`InProcessWorld`.

```rust
use bioprism_ids::RunId;
use bioprism_runtime::{
    DockerProcessConfig, DockerProcessSource, DockerSandbox, EffectKind, EffectPolicy,
    InProcessWorld, LinuxPlatform, RecordingHost, SandboxLimits,
};
use std::path::PathBuf;

fn configure() -> Result<(), Box<dyn std::error::Error>> {
let source = DockerProcessSource::new(
    DockerProcessConfig {
        image: format!("registry.example/tools/runner@sha256:{}", "a".repeat(64)),
        platform: LinuxPlatform::Amd64,
        input_dir: PathBuf::from("/data/task-input"),
        quarantine_root: PathBuf::from("/data/quarantine"),
        limits: SandboxLimits::default(),
    },
    DockerSandbox::default(),
    InProcessWorld::new(),
)?;
let policy = EffectPolicy::evaluation_default().declaring([EffectKind::ProcessSpawn]);
let _host = RecordingHost::new(RunId::parse("trial-1")?, source, policy);
Ok(())
}
```

The source bounds each captured stream to at most 1 MiB so a single process result remains
practical to record on the tape. Quarantine directories use mode `0700` on Unix; Windows applies
the quarantine root's inherited access-control rules, so operators should create that root with an
appropriate ACL.

This connects one effect seam; it does not implement the asynchronous `ExecutorProvider` lifecycle
or make plugin, workflow, or trial plans invoke Docker automatically.

## Use

The request names an already available Linux image by immutable SHA-256 digest, an absolute
executable path in that image, its arguments, an existing input directory, and a new output
directory path whose parent already exists. The image must provide `/bin/sh` and a `tar` command
that supports `--format=ustar`; the runner uses these fixed image tools to collect output without
interpolating the requested command or its arguments into shell source:

```rust
use bioprism_runtime::{DockerSandbox, LinuxPlatform, SandboxLimits, SandboxRequest};
use std::path::PathBuf;

fn execute() -> Result<(), Box<dyn std::error::Error>> {
let sandbox = DockerSandbox::default();
let version = sandbox.probe()?;
let result = sandbox.run(&SandboxRequest {
    image: format!("registry.example/tools/runner@sha256:{}", "a".repeat(64)),
    platform: LinuxPlatform::Amd64,
    command: "/usr/bin/python3".into(),
    arguments: vec!["/aurora/input/task.py".into()],
    input_dir: PathBuf::from("/data/task-input"),
    output_dir: PathBuf::from("/data/task-output"), // must not exist yet
    limits: SandboxLimits::default(),
})?;

if !result.cleanup_verified {
    return Err("Docker did not verify container cleanup".into());
}
if result.timed_out || result.output_limit_exceeded {
    return Err("sandbox command exceeded a configured bound".into());
}
println!("Docker Engine {version}; exit={:?}", result.exit_code);
Ok(())
}
```

The image must already exist locally: the runner uses `--pull=never` so execution never downloads
code as a side effect. `probe` resolves the selected Docker context or `DOCKER_HOST`, refuses
non-local endpoints, and pins subsequent commands to the resolved Unix socket, Windows named pipe,
or loopback TCP endpoint. This matters because Docker contexts and `DOCKER_HOST` select the daemon
and bind-mount paths are interpreted by that daemon. `run` rejects mutable image tags, non-Linux
platforms, missing directories, overlapping input/output trees, invalid limits, root container
identities, and images that declare Docker volumes before starting a container. It inspects the
requested platform's image configuration first and refuses unsupported or malformed volume metadata.
The image inspection uses Docker's `image inspect --platform` API option (API 1.49 or newer), so an
older Engine fails closed. It passes each argument directly to Docker without routing it through a
host shell.

## Enforced by the runner

Each invocation disables networking, uses private IPC, PID, and cgroup namespaces, sets a read-only
root filesystem, mounts input recursively read-only, drops Linux capabilities, enables
`no-new-privileges` and Docker's built-in seccomp profile, and runs under a non-root numeric
identity. Recursive read-only input mounts require Linux kernel 5.12 or newer on the Docker host;
older kernels fail closed when Docker applies the mount. CPU, RAM-without-swap,
processes, open files, `/tmp` and `/dev/shm` sizes, the tmpfs inode counts, wall-clock time,
per-stream stdout/stderr bytes, output
artifact bytes, and output entry count are bounded. `/aurora/output` and `/aurora/capture` are
size- and inode-limited tmpfs mounts, not host bind mounts. The fixed wrapper runs the requested
program as an argument vector, captures stdout to `/aurora/capture/stdout`, leaves stderr on the
bounded Docker stderr pipe, then streams a USTAR archive of the output and capture mounts. The host
parser accepts only the expected roots, regular files and directories, validates paths and checksums,
rejects links and special files, verifies aggregate bytes and entries, and hashes every output file.
Artifact paths reject traversal, absolute paths, Windows device names and invalid characters,
trailing dots or spaces, case-insensitive collisions, paths longer than 256 bytes, and directory depth
over 64. A success marker after the tar stream confirms that tar completed successfully before the
host attempts to parse or publish its output.
It publishes the complete output directory with one same-filesystem rename only after forced
container removal and absence verification succeed; cleanup also requests removal of anonymous
container volumes. Timeout, output overflow, malformed output, or unverified cleanup never publishes
partial artifacts. A per-run label supplements Docker's cidfile
so cleanup can find the container if the CLI is interrupted before returning its ID. Failures while
observing the Docker CLI use the same id-and-label cleanup path before the original error is
returned; an unverified cleanup takes precedence so a possibly live container is never hidden
behind a generic command error.

`SandboxLimits::max_artifact_bytes` defaults to 16 MiB and is capped at 64 MiB.
`max_artifact_entries` defaults to 1,024 and is capped at 4,096; entries count both files and
directories. `max_output_bytes` is a per-stream ceiling and defaults to 1 MiB. Returned
`DockerArtifact` rows are path-sorted and bind each published relative path to its exact byte count
and SHA-256 digest. The output, capture, and `/tmp` tmpfs mounts, plus the 16 MiB `/dev/shm` mount,
consume the container's memory budget. Include the selected artifact and stdout ceilings, 32 MiB of
fixed scratch space, and baseline process memory in the requested RAM limit. `/tmp` has a 16 MiB
size ceiling and 4,096-inode ceiling; `/dev/shm` is size-limited to 16 MiB.

## Host and image responsibilities

The daemon, its endpoint configuration, the host kernel, and the selected image remain trusted.
Endpoint validation rejects remote TCP and SSH contexts, but it cannot prove that a local loopback
socket is not a forwarding proxy. Pinning a digest makes the image reference immutable; it does not
make the image safe. The runner rejects image-declared Docker volumes, but callers still need to
review the image contents, entrypoint behavior, and tools that can write files.
The output parent remains caller-owned and writable. The runner refuses an output path that already
exists, preventing an execution from mixing new files with older caller data. The runner does not
inject host environment variables or stdin, but an image's own environment remains part of that
image. The required `/bin/sh` and `tar` executables are also part of the trusted image contents. The
tar reader limits portable artifact paths to the USTAR path capacity; an image tar implementation
that emits unsupported tar extensions fails closed. On Unix, staged output files are private to the
current user (mode `0600`) and directories are mode `0700`; on Windows the output inherits the
parent directory's access-control rules.

This boundary does not provide secret isolation for values embedded in the image, content scanning,
artifact release, independent result review, durable provenance, or an automatic policy/approval
gate. The explicit `DockerProcessSource` bridge leaves artifacts quarantined, but is not wired into
the autonomous agent or trial provider. On a host without a reachable Docker Engine,
`probe` and `run` fail closed; this checkout's Windows Docker Desktop Linux engine was unavailable
during implementation, so a real-container integration run could not be performed here.
