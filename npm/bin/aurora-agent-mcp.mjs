#!/usr/bin/env node
// Launcher for the AURORA Agent (bioprism) MCP server.
//
// Resolution order for the server binary:
//   1. AURORA_AGENT_MCP_EXE  — explicit path to bioprism-mcp(.exe)
//   2. AURORA_AGENT_ROOT     — a checkout with target/release/bioprism-mcp(.exe)
//   3. ~/aurora-agent, ~/bioprism — conventional checkout locations
//   4. A cached copy of the signed release bundle, downloaded once from
//      GitHub Releases and verified against a pinned SHA-256 before first use.
//
// The data root (--root) defaults to the checkout (cases 2-3) or the bundled
// reference fixtures (case 4); AURORA_AGENT_ROOT always wins when set.
//
// stdout belongs to the MCP stdio channel; every diagnostic goes to stderr.

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync, copyFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

const BUNDLE_VERSION = "0.1.1";
const BUNDLE_URL =
  "https://github.com/MurariAmbati/aurora-agent-releases/releases/download/v0.1.1/aurora-agent.mcpb";
const BUNDLE_SHA256 =
  "e987c396a01eb2ae812efa8226f6892fc3f70cb441102217476990816458f025";

const log = (msg) => process.stderr.write(`[aurora-agent-mcp] ${msg}\n`);

function fail(msg) {
  log(msg);
  process.exit(1);
}

function exeCandidates(root) {
  return [
    join(root, "target", "release", "bioprism-mcp.exe"),
    join(root, "target", "release", "bioprism-mcp"),
  ];
}

function resolveFromCheckout() {
  const roots = [];
  if (process.env.AURORA_AGENT_ROOT) roots.push(process.env.AURORA_AGENT_ROOT);
  roots.push(join(homedir(), "aurora-agent"), join(homedir(), "bioprism"));
  for (const root of roots) {
    for (const exe of exeCandidates(root)) {
      if (existsSync(exe)) return { exe, root };
    }
  }
  return null;
}

function cacheDir() {
  const base =
    process.env.LOCALAPPDATA && process.platform === "win32"
      ? process.env.LOCALAPPDATA
      : join(homedir(), ".cache");
  return join(base, "aurora-agent-mcp", `v${BUNDLE_VERSION}`);
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function extract(archive, dest) {
  mkdirSync(dest, { recursive: true });
  // Prefer the system bsdtar on Windows: a GNU tar earlier in PATH (Git Bash)
  // treats "C:" as a remote host and cannot read zip archives anyway.
  const sysTar = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
  const tarCmd = process.platform === "win32" && existsSync(sysTar) ? sysTar : "tar";
  const tar = spawnSync(tarCmd, ["-xf", archive, "-C", dest], { stdio: ["ignore", "ignore", "pipe"] });
  if (tar.status === 0) return;
  // tar.exe ships with Windows 10+; if it is unavailable fall back to Expand-Archive,
  // which insists on a .zip extension (.mcpb is a plain zip).
  const zip = `${archive}.zip`;
  copyFileSync(archive, zip);
  const ps = spawnSync(
    "powershell",
    ["-NoProfile", "-Command", `Expand-Archive -LiteralPath '${zip}' -DestinationPath '${dest}' -Force`],
    { stdio: ["ignore", "ignore", "inherit"] }
  );
  rmSync(zip, { force: true });
  if (ps.status !== 0) throw new Error("could not extract the release bundle (tar and Expand-Archive both failed)");
}

async function resolveFromBundle() {
  const dir = cacheDir();
  const exe = join(dir, "server", "bioprism-mcp.exe");
  if (existsSync(exe)) return { exe, root: process.env.AURORA_AGENT_ROOT ?? join(dir, "root") };

  log(`downloading AURORA Agent ${BUNDLE_VERSION} release bundle (one-time, ~15 MB)...`);
  mkdirSync(dirname(dir), { recursive: true });
  const archive = join(dirname(dir), `aurora-agent-${BUNDLE_VERSION}.mcpb.tmp`);
  const res = await fetch(BUNDLE_URL);
  if (!res.ok) throw new Error(`download failed: HTTP ${res.status} for ${BUNDLE_URL}`);
  writeFileSync(archive, Buffer.from(await res.arrayBuffer()));

  const digest = sha256(archive);
  if (digest !== BUNDLE_SHA256) {
    rmSync(archive, { force: true });
    throw new Error(`SHA-256 mismatch for downloaded bundle: got ${digest}, expected ${BUNDLE_SHA256}; refusing to run it`);
  }

  const partial = `${dir}.partial`;
  rmSync(partial, { recursive: true, force: true });
  extract(archive, partial);
  rmSync(archive, { force: true });
  if (!existsSync(join(partial, "server", "bioprism-mcp.exe"))) {
    throw new Error("release bundle did not contain server/bioprism-mcp.exe");
  }
  rmSync(dir, { recursive: true, force: true });
  renameSync(partial, dir);
  log(`bundle verified (sha256 ${BUNDLE_SHA256.slice(0, 12)}...) and cached at ${dir}`);
  return { exe, root: process.env.AURORA_AGENT_ROOT ?? join(dir, "root") };
}

async function main() {
  let exe;
  let root;

  if (process.env.AURORA_AGENT_MCP_EXE) {
    exe = process.env.AURORA_AGENT_MCP_EXE;
    if (!existsSync(exe)) fail(`AURORA_AGENT_MCP_EXE points at a missing file: ${exe}`);
    root = process.env.AURORA_AGENT_ROOT ?? process.cwd();
  } else {
    const local = resolveFromCheckout();
    if (local) {
      ({ exe, root } = local);
      root = process.env.AURORA_AGENT_ROOT ?? root;
    } else if (process.platform !== "win32") {
      fail(
        "no local bioprism-mcp binary found, and the prebuilt release bundle is Windows-only for now.\n" +
          "  On this platform, build from source (cargo build --release) in a clone of\n" +
          "  https://github.com/AURORA-NEURO/aurora-agent and set AURORA_AGENT_ROOT to the checkout."
      );
    } else {
      try {
        ({ exe, root } = await resolveFromBundle());
      } catch (err) {
        fail(`${err.message}\n  Alternative: clone https://github.com/AURORA-NEURO/aurora-agent, run cargo build --release, and set AURORA_AGENT_ROOT.`);
      }
    }
  }

  const args = ["--root", root, ...process.argv.slice(2)];
  const child = spawn(exe, args, { stdio: "inherit" });
  child.on("error", (err) => fail(`failed to start ${exe}: ${err.message}`));
  child.on("exit", (code, signal) => process.exit(signal ? 1 : code ?? 1));
}

main();
