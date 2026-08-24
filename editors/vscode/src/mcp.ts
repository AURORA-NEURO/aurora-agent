import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { exeName } from "./resolveCore";
import { Resolver, downloadAndCache } from "./resolve";
import { RELEASE_VERSION, platformArchive } from "./pins";

const PROVIDER_ID = "aurora-agent";

interface McpTarget {
  command: string;
  root: string;
  rootSource: "resolved" | "binary-dir" | "cache";
}

function currentTarget(resolver: Resolver): McpTarget | undefined {
  const resolved = resolver.resolve();
  if (resolved && fs.existsSync(resolved.bioprismMcp)) {
    if (resolved.root !== undefined) {
      return { command: resolved.bioprismMcp, root: resolved.root, rootSource: "resolved" };
    }
    return { command: resolved.bioprismMcp, root: path.dirname(resolved.bioprismMcp), rootSource: "binary-dir" };
  }
  return undefined;
}

function expectedCacheTarget(resolver: Resolver): McpTarget | undefined {
  if (platformArchive(process.platform, process.arch) === undefined) {
    return undefined;
  }
  const dir = resolver.cacheVersionDir();
  return { command: path.join(dir, exeName("bioprism-mcp", process.platform)), root: dir, rootSource: "cache" };
}

export function describeMcpTarget(resolver: Resolver): string {
  const target = currentTarget(resolver) ?? expectedCacheTarget(resolver);
  if (!target) {
    return "MCP server --root: (none — no bioprism-mcp binary resolved and no prebuilt archive for this platform)";
  }
  const sourceText =
    target.rootSource === "resolved"
      ? "from binary resolution"
      : target.rootSource === "binary-dir"
        ? "no root resolved; falling back to the bioprism-mcp binary's directory"
        : "download cache (binary not present yet)";
  return `MCP server --root: ${target.root} (${sourceText})`;
}

function definitionFor(target: McpTarget): vscode.McpStdioServerDefinition {
  const definition = new vscode.McpStdioServerDefinition(
    "AURORA Agent",
    target.command,
    ["--root", target.root],
    {},
    RELEASE_VERSION
  );
  definition.cwd = vscode.Uri.file(path.dirname(target.command));
  return definition;
}

export function registerMcpProvider(
  context: vscode.ExtensionContext,
  resolver: Resolver,
  log: (line: string) => void
): boolean {
  const lm = vscode.lm as typeof vscode.lm & {
    registerMcpServerDefinitionProvider?: typeof vscode.lm.registerMcpServerDefinitionProvider;
  };
  if (typeof lm.registerMcpServerDefinitionProvider !== "function") {
    log("vscode.lm.registerMcpServerDefinitionProvider is unavailable in this build; MCP registration skipped");
    return false;
  }

  const changed = new vscode.EventEmitter<void>();
  context.subscriptions.push(changed);
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("auroraAgent")) {
        changed.fire();
      }
    })
  );

  const provider: vscode.McpServerDefinitionProvider<vscode.McpStdioServerDefinition> = {
    onDidChangeMcpServerDefinitions: changed.event,
    provideMcpServerDefinitions(): vscode.McpStdioServerDefinition[] {
      const target = currentTarget(resolver) ?? expectedCacheTarget(resolver);
      if (!target) {
        return [];
      }
      return [definitionFor(target)];
    },
    async resolveMcpServerDefinition(
      server: vscode.McpStdioServerDefinition
    ): Promise<vscode.McpStdioServerDefinition | undefined> {
      const live = currentTarget(resolver);
      if (live) {
        return definitionFor(live);
      }
      const choice = await vscode.window.showInformationMessage(
        `AURORA Agent MCP server: the bioprism-mcp binary is not present. Download the v${RELEASE_VERSION} release archive (SHA-256 verified) into the extension cache?`,
        { modal: true },
        "Download"
      );
      if (choice !== "Download") {
        return undefined;
      }
      await downloadAndCache(resolver.cacheVersionDir(), log);
      const after = currentTarget(resolver);
      if (!after) {
        throw new Error("the downloaded archive did not contain bioprism-mcp in an expected layout");
      }
      return definitionFor(after);
    },
  };

  context.subscriptions.push(lm.registerMcpServerDefinitionProvider(PROVIDER_ID, provider));
  log(`registered MCP server definition provider '${PROVIDER_ID}'`);
  return true;
}

export function buildMcpJsonSnippet(command: string, root: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        "aurora-agent": {
          command,
          args: ["--root", root],
        },
      },
    },
    null,
    2
  );
}

export async function copyMcpJson(resolver: Resolver): Promise<void> {
  const target = currentTarget(resolver) ?? expectedCacheTarget(resolver);
  if (!target) {
    void vscode.window.showErrorMessage(
      "AURORA Agent: no bioprism-mcp binary resolved and no prebuilt archive exists for this platform. " +
        "Set auroraAgent.binaryDir to a build of the binaries first."
    );
    return;
  }
  const snippet = buildMcpJsonSnippet(target.command, target.root);
  await vscode.env.clipboard.writeText(snippet);
  const exists = fs.existsSync(target.command);
  void vscode.window.showInformationMessage(
    exists
      ? "AURORA Agent: .mcp.json snippet copied to the clipboard."
      : "AURORA Agent: .mcp.json snippet copied, but the binary at that path does not exist yet — run 'AURORA Agent: Status: Doctor' to download or configure it."
  );
}
