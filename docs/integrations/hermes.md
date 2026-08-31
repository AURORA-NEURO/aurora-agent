# Hermes MCP integration contract

`integrations/hermes` is a dependency-free configuration adapter for Hermes-style `mcp_servers`
entries. It validates and normalizes configuration without resolving environment variables or
storing credentials.

## Supported configuration

Stdio entries use an argv contract:

```yaml
mcp_servers:
  aurora:
    command: bioprism-mcp
    args: [--root, '${workspaceFolder}']
    env:
      AURORA_TOKEN: '${AURORA_TOKEN}'
```

HTTP entries use an absolute `http` or `https` URL and optional headers. Secret-bearing headers and
environment values must be full-string placeholders such as `${AURORA_TOKEN}`. The adapter never
expands them. Literal values under names such as `TOKEN`, `SECRET`, `PASSWORD`, `API_KEY`, or
`AUTHORIZATION` are rejected and diagnostic redaction never repeats them.

Aurora's local MCP launch shape is:

```text
./target/release/bioprism-mcp --root .
```

The root repository's `.mcp.json` is the source of truth for the exact local command. On Windows,
use the built binary path appropriate for the checkout; the validator does not synthesize shell
commands or invoke a shell.

## Diagnostics and limits

Validation is offline. `diagnose(spec)` returns `unmeasured` unless a caller explicitly requests a
probe. A stdio probe checks whether the executable can be found but never starts it. HTTP probing
is reported as `unsupported` because URL syntax is not network readiness. This package does not
implement OAuth, auth token acquisition, SSE, arbitrary transports, remote execution, retries, or
credential rotation; those requests are typed refusals or configuration errors.

JSON serialization is canonical and secret-safe. YAML load/dump is optional and requires PyYAML;
the core package has no runtime dependency. The output keeps placeholders byte-for-byte visible so
Hermes can resolve them in its own profile scope.

## Verification

From `integrations/hermes`:

```text
python -m unittest discover -s tests -t . -v
python -m compileall -q aurora_hermes tests
```
