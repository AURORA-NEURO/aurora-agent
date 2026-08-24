#!/usr/bin/env node
// Bootstrap launcher for the aurora-agent MCP server (bioprism-mcp).
//
// The plugin cannot assume where the aurora-agent checkout lives or that the
// process cwd is the repo root (the repo's own .mcp.json assumes both), so
// this script resolves the backend root, verifies the built binary, and execs
// it with --root <root>. stdio is passed straight through: stdout stays pure
// JSON-RPC, stderr carries diagnostics.
//
// Root resolution order:
//   1. AURORA_AGENT_ROOT environment variable
//   2. ~/aurora-agent
//   3. ~/bioprism
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const candidates = [
  process.env.AURORA_AGENT_ROOT,
  join(homedir(), "aurora-agent"),
  join(homedir(), "bioprism"),
].filter(Boolean);

let root = null;
for (const candidate of candidates) {
  if (existsSync(join(candidate, "Cargo.toml"))) {
    root = candidate;
    break;
  }
}

if (!root) {
  process.stderr.write(
    "aurora-backend: no aurora-agent checkout found.\n" +
      `Searched: ${candidates.join(", ")}\n` +
      "Clone the repo and set AURORA_AGENT_ROOT, or place it at ~/aurora-agent.\n",
  );
  process.exit(1);
}

const exe = process.platform === "win32" ? "bioprism-mcp.exe" : "bioprism-mcp";
const binary = join(root, "target", "release", exe);
if (!existsSync(binary)) {
  process.stderr.write(
    `aurora-backend: MCP binary not built: ${binary}\n` +
      `Build it with:  cd "${root}" && cargo build --release --offline -p bioprism-mcp\n` +
      "(If a freshly built test binary later reports os error 4551, Windows Application\n" +
      " Control blocked it — touch a source file to force a relink and rebuild.)\n",
  );
  process.exit(1);
}

const child = spawn(binary, ["--root", root], { stdio: "inherit" });
child.on("exit", (code, signal) => {
  process.exit(signal ? 1 : code ?? 1);
});
child.on("error", (err) => {
  process.stderr.write(`aurora-backend: failed to start ${binary}: ${err.message}\n`);
  process.exit(1);
});
