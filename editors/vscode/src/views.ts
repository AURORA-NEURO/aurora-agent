import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runCli, runPlain } from "./cli";
import { describeOutcome } from "./envelope";
import { parseCatalogue, CatalogueGroup } from "./catalogue";
import { mapGhRuns, parseRemoteToNwo, GhRun } from "./ghruns";
import { Resolver } from "./resolve";

export function reportsDirectory(): string | undefined {
  const configured = vscode.workspace.getConfiguration("auroraAgent").get<string>("reportsDir");
  if (configured && configured.trim() !== "") {
    return configured.trim();
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    return undefined;
  }
  return path.join(folder.uri.fsPath, ".aurora", "reports");
}

export class WorkflowItem extends vscode.TreeItem {
  constructor(readonly group: CatalogueGroup) {
    super(group.id, vscode.TreeItemCollapsibleState.Collapsed);
    this.contextValue = "workflowGroup";
    this.description = group.toolsMissing.length > 0 ? `${group.status}, ${group.toolsMissing.length} tools missing` : group.status;
    this.tooltip = [
      group.title,
      `domains: ${group.domains.join(", ") || "(none)"}`,
      `tools declared: ${group.toolsDeclared.length}, available: ${group.toolsAvailable.length}, missing: ${group.toolsMissing.length}`,
    ].join("\n");
    this.iconPath = new vscode.ThemeIcon("symbol-namespace");
  }
}

class WorkflowToolItem extends vscode.TreeItem {
  constructor(tool: string, missing: boolean) {
    super(tool, vscode.TreeItemCollapsibleState.None);
    this.contextValue = "workflowTool";
    this.iconPath = missing
      ? new vscode.ThemeIcon("warning", new vscode.ThemeColor("problemsWarningIcon.foreground"))
      : new vscode.ThemeIcon("tools");
    this.description = missing ? "missing from this build" : undefined;
  }
}

class MessageItem extends vscode.TreeItem {
  constructor(message: string, tooltip?: string) {
    super(message, vscode.TreeItemCollapsibleState.None);
    this.iconPath = new vscode.ThemeIcon("info");
    if (tooltip) {
      this.tooltip = tooltip;
    }
  }
}

export class WorkflowsProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private cached: CatalogueGroup[] | undefined;
  private failure: string | undefined;

  constructor(private readonly resolver: Resolver, private readonly channel: vscode.OutputChannel) {}

  refresh(): void {
    this.cached = undefined;
    this.failure = undefined;
    this.changed.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: vscode.TreeItem): Promise<vscode.TreeItem[]> {
    if (element instanceof WorkflowItem) {
      const available = element.group.toolsAvailable.map((tool) => new WorkflowToolItem(tool, false));
      const missing = element.group.toolsMissing.map((tool) => new WorkflowToolItem(tool, true));
      return [...available, ...missing];
    }
    if (element) {
      return [];
    }
    if (this.failure !== undefined) {
      return [new MessageItem(this.failure)];
    }
    if (this.cached === undefined) {
      const resolved = this.resolver.resolve();
      if (!resolved) {
        return [new MessageItem("No bioprism binary resolved — run 'AURORA Agent: Status: Doctor'.")];
      }
      const outcome = await runCli(resolved.bioprism, ["workflow", "catalogue"], this.channel, {
        cwd: resolved.root,
      });
      if (outcome.kind !== "ok" && outcome.kind !== "verdict") {
        this.failure = `workflow catalogue failed: ${describeOutcome(outcome)}`;
        return [new MessageItem(this.failure)];
      }
      this.cached = parseCatalogue(outcome.document);
    }
    if (this.cached.length === 0) {
      return [new MessageItem("The catalogue returned no capability groups.")];
    }
    return this.cached.map((group) => new WorkflowItem(group));
  }

  groups(): CatalogueGroup[] {
    return this.cached ?? [];
  }
}

export class ReportItem extends vscode.TreeItem {
  constructor(readonly filePath: string, finalStatus: string | undefined) {
    super(path.basename(filePath), vscode.TreeItemCollapsibleState.None);
    this.contextValue = "autopilotReport";
    this.description = finalStatus ?? "no final_status field";
    switch (finalStatus) {
      case "succeeded":
        this.iconPath = new vscode.ThemeIcon("pass", new vscode.ThemeColor("testing.iconPassed"));
        break;
      case "exhausted":
        this.iconPath = new vscode.ThemeIcon("warning", new vscode.ThemeColor("problemsWarningIcon.foreground"));
        break;
      case "refused":
        this.iconPath = new vscode.ThemeIcon("circle-slash", new vscode.ThemeColor("problemsErrorIcon.foreground"));
        break;
      default:
        this.iconPath = new vscode.ThemeIcon("question");
        break;
    }
    this.command = {
      command: "auroraAgent.openReport",
      title: "Open Autopilot Report Summary",
      arguments: [filePath],
    };
  }
}

export class ReportsProvider implements vscode.TreeDataProvider<vscode.TreeItem>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private watcher: fs.FSWatcher | undefined;
  private watchedDir: string | undefined;
  private debounce: NodeJS.Timeout | undefined;

  refresh(): void {
    this.changed.fire();
  }

  dispose(): void {
    this.watcher?.close();
    if (this.debounce) {
      clearTimeout(this.debounce);
    }
    this.changed.dispose();
  }

  private ensureWatcher(dir: string): void {
    if (this.watchedDir === dir) {
      return;
    }
    this.watcher?.close();
    this.watcher = undefined;
    this.watchedDir = dir;
    try {
      this.watcher = fs.watch(dir, () => {
        if (this.debounce) {
          clearTimeout(this.debounce);
        }
        this.debounce = setTimeout(() => this.changed.fire(), 300);
      });
      this.watcher.on("error", () => {
        this.watcher?.close();
        this.watcher = undefined;
        this.watchedDir = undefined;
      });
    } catch {
      this.watcher = undefined;
    }
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: vscode.TreeItem): vscode.TreeItem[] {
    if (element) {
      return [];
    }
    const dir = reportsDirectory();
    if (!dir) {
      return [new MessageItem("Open a folder (or set auroraAgent.reportsDir) to list autopilot reports.")];
    }
    if (!fs.existsSync(dir)) {
      return [
        new MessageItem(
          `No reports directory yet (${dir}).`,
          "The directory is created when 'Autopilot: Run' writes its first --report-out file."
        ),
      ];
    }
    this.ensureWatcher(dir);
    let entries: string[];
    try {
      entries = fs.readdirSync(dir).filter((name) => name.endsWith(".json"));
    } catch (error) {
      return [new MessageItem(`Could not read ${dir}: ${error instanceof Error ? error.message : String(error)}`)];
    }
    if (entries.length === 0) {
      return [new MessageItem("No report JSON files in the reports directory yet.")];
    }
    const items = entries
      .map((name) => {
        const filePath = path.join(dir, name);
        let finalStatus: string | undefined;
        try {
          const parsed: unknown = JSON.parse(fs.readFileSync(filePath, "utf8"));
          if (parsed && typeof parsed === "object") {
            const value = (parsed as Record<string, unknown>)["final_status"];
            if (typeof value === "string") {
              finalStatus = value;
            }
          }
        } catch {
          finalStatus = undefined;
        }
        let mtime = 0;
        try {
          mtime = fs.statSync(filePath).mtimeMs;
        } catch {
          mtime = 0;
        }
        return { item: new ReportItem(filePath, finalStatus), mtime };
      })
      .sort((a, b) => b.mtime - a.mtime)
      .map((entry) => entry.item);
    return items;
  }
}

class RunItem extends vscode.TreeItem {
  constructor(run: GhRun) {
    super(run.workflow || run.title || run.id, vscode.TreeItemCollapsibleState.None);
    this.contextValue = "pipelineRun";
    const state = run.conclusion || run.status;
    this.description = run.conclusion ? `${run.status}, ${run.conclusion}` : run.status;
    this.tooltip = [run.title, `status: ${run.status}`, `conclusion: ${run.conclusion || "(none yet)"}`, run.createdAt]
      .filter((line) => line !== "")
      .join("\n");
    switch (state) {
      case "success":
        this.iconPath = new vscode.ThemeIcon("pass", new vscode.ThemeColor("testing.iconPassed"));
        break;
      case "failure":
        this.iconPath = new vscode.ThemeIcon("error", new vscode.ThemeColor("problemsErrorIcon.foreground"));
        break;
      case "in_progress":
      case "queued":
      case "pending":
      case "waiting":
        this.iconPath = new vscode.ThemeIcon("sync");
        break;
      default:
        this.iconPath = new vscode.ThemeIcon("circle-outline");
        break;
    }
    if (run.url !== "") {
      this.command = {
        command: "auroraAgent.openRun",
        title: "Open Pipeline Run in Browser",
        arguments: [run.url],
      };
    }
  }
}

export class PipelinesProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;

  constructor(private readonly channel: vscode.OutputChannel) {}

  refresh(): void {
    this.changed.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: vscode.TreeItem): Promise<vscode.TreeItem[]> {
    if (element) {
      return [];
    }
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      return [new MessageItem("Open a folder with a GitHub remote to list Actions runs.")];
    }
    const remote = await runPlain("git", ["config", "--get", "remote.origin.url"], this.channel, {
      cwd: folder.uri.fsPath,
    });
    if (remote.exitCode === null) {
      return [new MessageItem("git is not on PATH — install it to detect the workspace's GitHub remote.")];
    }
    if (remote.exitCode !== 0) {
      return [new MessageItem("No git remote.origin.url in this workspace.")];
    }
    const nwo = parseRemoteToNwo(remote.stdout);
    if (!nwo) {
      return [new MessageItem("remote.origin.url is not a github.com repository.")];
    }
    const result = await runPlain(
      "gh",
      [
        "run",
        "list",
        "-R",
        nwo,
        "--limit",
        "10",
        "--json",
        "databaseId,name,displayTitle,status,conclusion,url,createdAt",
      ],
      this.channel,
      { cwd: folder.uri.fsPath }
    );
    if (result.exitCode === null) {
      return [new MessageItem("GitHub CLI (gh) is not on PATH — install it to see Actions runs here.")];
    }
    if (result.exitCode !== 0) {
      const firstLine = result.stderr.split(/\r?\n/).find((line) => line.trim() !== "") ?? "unknown error";
      return [new MessageItem(`gh could not list runs (${firstLine.trim()}). Run 'gh auth login' if unauthenticated.`)];
    }
    const runs = mapGhRuns(result.stdout);
    if (runs.length === 0) {
      return [new MessageItem(`No Actions runs found for ${nwo}.`)];
    }
    return runs.map((run) => new RunItem(run));
  }
}
