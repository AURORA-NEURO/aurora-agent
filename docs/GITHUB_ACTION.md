# Autonomous GitHub Action

The reusable composite action at `.github/actions/autonomous-run` invokes the Python
`aurora-agent run` boundary inside a GitHub Actions workflow. It provides a consumer-repository
entry point while leaving the model provider, credential, and MCP workspace under the workflow
author's control.

```yaml
name: Aurora review
on: [workflow_dispatch]

permissions:
  contents: read

jobs:
  agent:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - id: aurora
        uses: AURORA-NEURO/aurora-agent/.github/actions/autonomous-run@<reviewed-ref>
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
        with:
          task: Review the checked out project and summarize the highest-priority issue.
          mcp-command: python -m your_project_mcp_server
          provider: openai
          models: gpt-4.1
          approve-provider-call: 'true'
      - name: Require a completed provider result
        if: steps.aurora.outputs.status != 'completed_provider_call'
        run: exit 1
```

Supply `models` as one provider model name per line to offer several candidates. Leave `domain`
empty for automatic routing or name one reviewed domain. `hints` is also newline-separated.
Provider approval is false by default. Mission dispatch has a separate false-by-default approval,
and `tool_loop`/`mission` modes require an exact `allow-mcp-tools` list. MCP calls use a configurable
timeout from 1 to 600 seconds (30 by default). The action passes the
MCP command to the Python SDK, which tokenizes it and starts it without a shell. The MCP server
must be available in the caller's runner workspace and receive any required credentials through
its own environment. The action removes its `INPUT_*` variables and the configured provider-key
variable while the CLI starts the MCP child process; if the MCP server needs a credential, provide
it under a separate caller-owned environment variable.

The action captures the CLI JSON in memory and writes only a bounded status, routing mode, run ID,
and SHA-256 digest to `GITHUB_OUTPUT` and the workflow log. It does not upload or persist a result
file by default. Set `result-file` only when the workflow owner wants the full JSON response in a
path inside `GITHUB_WORKSPACE`; that file can contain the task and model response and follows the
caller's workspace retention policy. The digest is over the exact CLI JSON bytes and does not
certify task correctness or scientific validity. A nonzero CLI process result fails the action;
domain outcomes such as `paused`, `blocked`, or `approval_required` are exposed as `status` so a
workflow can define its own gate. Successful provider-only and tool-loop calls have distinct
statuses (`completed_provider_call` and `completed_provider_tool_loop`); gate on the result modes
enabled in that workflow.

The action requires Python 3.11 or newer and uses the dependency-free source SDK shipped beside
the action. The workflow author must pin a reviewed action revision, provide the MCP executable,
map secrets only to the named environment variables, and choose the provider and mission
approvals appropriate to that workflow.

The repository's Python CI exercises the action end to end against a fixture MCP process and the
credentialless local provider. That verifies checkout-relative source loading, CLI invocation,
MCP process lifecycle, outputs, and default omission of full result content without contacting an
external model provider.
