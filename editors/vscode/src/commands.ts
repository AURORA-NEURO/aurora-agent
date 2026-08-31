import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { Resolver, ResolvedBinaries } from "./resolve";
import { runCli, runPlain, notifyOutcome } from "./cli";
import { CliOutcome } from "./envelope";
import { SummaryProvider } from "./render";
import { WorkflowsProvider, ReportsProvider, PipelinesProvider, WorkflowItem, reportsDirectory } from "./views";
import { summarizeGrant, grantConfirmationText } from "./grants";
import { parseCatalogue, CatalogueGroup } from "./catalogue";
import { copyMcpJson, describeMcpTarget } from "./mcp";

export interface CommandDeps {
  context: vscode.ExtensionContext;
  resolver: Resolver;
  channel: vscode.OutputChannel;
  summaries: SummaryProvider;
  workflows: WorkflowsProvider;
  reports: ReportsProvider;
  pipelines: PipelinesProvider;
  mcpAvailable: boolean;
  onBinaryStateMaybeChanged: () => void;
}

function timestamp(): string {
  return new Date().toISOString().replace(/[:.]/g, "-").replace("T", "-").slice(0, 19);
}

async function requireBinary(deps: CommandDeps): Promise<ResolvedBinaries | undefined> {
  const resolved = await deps.resolver.resolveOrOfferDownload();
  deps.onBinaryStateMaybeChanged();
  return resolved;
}

async function pickJsonFile(title: string, defaultDir?: string): Promise<string | undefined> {
  const options: vscode.OpenDialogOptions = {
    title,
    canSelectMany: false,
    filters: { JSON: ["json"] },
    openLabel: title,
  };
  if (defaultDir && fs.existsSync(defaultDir)) {
    options.defaultUri = vscode.Uri.file(defaultDir);
  }
  const picked = await vscode.window.showOpenDialog(options);
  return picked?.[0]?.fsPath;
}

function auroraDir(): string | undefined {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    return undefined;
  }
  const dir = path.join(folder.uri.fsPath, ".aurora");
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

async function runWithProgress(
  title: string,
  work: () => Promise<CliOutcome>
): Promise<CliOutcome> {
  return vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title },
    () => work()
  );
}

async function openUntitledJson(content: unknown): Promise<void> {
  const doc = await vscode.workspace.openTextDocument({
    language: "json",
    content: JSON.stringify(content, null, 2) + "\n",
  });
  await vscode.window.showTextDocument(doc);
}

function documentLooksLikeReport(document: unknown): boolean {
  return (
    typeof document === "object" &&
    document !== null &&
    typeof (document as Record<string, unknown>)["final_status"] === "string"
  );
}

async function pickWorkflowGroup(deps: CommandDeps, item?: WorkflowItem): Promise<CatalogueGroup | undefined> {
  if (item instanceof WorkflowItem) {
    return item.group;
  }
  let groups = deps.workflows.groups();
  if (groups.length === 0) {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return undefined;
    }
    const outcome = await runCli(resolved.bioprism, ["workflow", "catalogue"], deps.channel, { cwd: resolved.root });
    if (outcome.kind !== "ok" && outcome.kind !== "verdict") {
      notifyOutcome(outcome, "workflow catalogue", deps.channel);
      return undefined;
    }
    groups = parseCatalogue(outcome.document);
  }
  if (groups.length === 0) {
    void vscode.window.showWarningMessage("AURORA Agent: the workflow catalogue lists no capability groups.");
    return undefined;
  }
  const picked = await vscode.window.showQuickPick(
    groups.map((group) => ({
      label: group.id,
      description: group.status,
      detail: `${group.title} — tools available: ${group.toolsAvailable.length}, missing: ${group.toolsMissing.length}`,
      group,
    })),
    { title: "Select a capability group", matchOnDetail: true }
  );
  return picked?.group;
}

async function askMissionInputs(): Promise<{ missionId: string; goal: string } | undefined> {
  const missionId = await vscode.window.showInputBox({
    title: "Mission id",
    prompt: "Identifier for this mission (letters, digits, . _ -)",
    validateInput: (value) =>
      /^[A-Za-z0-9._-]+$/.test(value) ? undefined : "Use letters, digits, dots, underscores, or hyphens.",
  });
  if (missionId === undefined) {
    return undefined;
  }
  const goal = await vscode.window.showInputBox({
    title: "Mission goal",
    prompt: "One-sentence goal for the mission",
    validateInput: (value) => (value.trim() === "" ? "The goal must not be empty." : undefined),
  });
  if (goal === undefined) {
    return undefined;
  }
  return { missionId, goal: goal.trim() };
}

export function registerCommands(deps: CommandDeps): void {
  const { context, channel, summaries } = deps;
  const register = (command: string, callback: (...args: unknown[]) => unknown) => {
    context.subscriptions.push(vscode.commands.registerCommand(command, callback));
  };

  register("auroraAgent.compile", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const world = await pickJsonFile("Select world JSON", resolved.root);
    if (!world) {
      return;
    }
    const query = await pickJsonFile("Select query JSON", path.dirname(world));
    if (!query) {
      return;
    }
    const outDir = auroraDir();
    if (!outDir) {
      void vscode.window.showErrorMessage("AURORA Agent: open a folder first — the certificate is written into <folder>/.aurora/.");
      return;
    }
    const stamp = timestamp();
    const certificateOut = path.join(outDir, `certificate-${stamp}.json`);
    const sectionOut = path.join(outDir, `section-${stamp}.json`);
    const outcome = await runWithProgress("AURORA Agent: compiling context", () =>
      runCli(
        resolved.bioprism,
        [
          "context",
          "compile",
          "--world",
          world,
          "--query",
          query,
          "--certificate-out",
          certificateOut,
          "--section-out",
          sectionOut,
        ],
        channel,
        { cwd: resolved.root }
      )
    );
    notifyOutcome(outcome, "context compile", channel);
    if (fs.existsSync(certificateOut)) {
      await summaries.openFileSummary(certificateOut, "certificate");
    } else if (outcome.document !== undefined) {
      await summaries.openDocumentSummary("Context compile outcome", outcome.document, "generic");
    }
  });

  register("auroraAgent.verifyCertificate", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const certificate = await pickJsonFile("Select certificate JSON", auroraDir());
    if (!certificate) {
      return;
    }
    const outcome = await runWithProgress("AURORA Agent: verifying certificate", () =>
      runCli(resolved.bioprism, ["context", "verify", "--certificate", certificate], channel, { cwd: resolved.root })
    );
    notifyOutcome(outcome, "context verify", channel);
    if (outcome.document !== undefined) {
      await summaries.openDocumentSummary("Certificate verification", outcome.document, "generic");
    }
  });

  register("auroraAgent.explainPlan", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const world = await pickJsonFile("Select world JSON", resolved.root);
    if (!world) {
      return;
    }
    const query = await pickJsonFile("Select query JSON", path.dirname(world));
    if (!query) {
      return;
    }
    const outcome = await runWithProgress("AURORA Agent: explaining compile plan", () =>
      runCli(resolved.bioprism, ["context", "explain", "--world", world, "--query", query], channel, {
        cwd: resolved.root,
      })
    );
    notifyOutcome(outcome, "context explain", channel);
    if (outcome.document !== undefined) {
      await summaries.openDocumentSummary("Compile plan explanation", outcome.document, "generic");
    }
  });

  register("auroraAgent.validateWorld", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const active = vscode.window.activeTextEditor?.document;
    let world: string | undefined;
    if (active && active.uri.scheme === "file" && active.fileName.endsWith(".json")) {
      world = active.fileName;
    } else {
      world = await pickJsonFile("Select world JSON", resolved.root);
    }
    if (!world) {
      return;
    }
    const outcome = await runWithProgress("AURORA Agent: validating world", () =>
      runCli(resolved.bioprism, ["world", "validate", "--world", world], channel, { cwd: resolved.root })
    );
    notifyOutcome(outcome, "world validate", channel);
    if (outcome.document !== undefined) {
      await summaries.openDocumentSummary("World validation", outcome.document, "generic");
    }
  });

  register("auroraAgent.browseCatalogue", async () => {
    await vscode.commands.executeCommand("auroraAgent.workflows.focus");
  });

  register("auroraAgent.scaffoldWorkflow", async (item) => {
    const group = await pickWorkflowGroup(deps, item as WorkflowItem | undefined);
    if (!group) {
      return;
    }
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const inputs = await askMissionInputs();
    if (!inputs) {
      return;
    }
    const outcome = await runWithProgress(`AURORA Agent: scaffolding ${group.id}`, () =>
      runCli(
        resolved.bioprism,
        ["workflow", "scaffold", "--workflow", group.id, "--mission-id", inputs.missionId, "--goal", inputs.goal],
        channel,
        { cwd: resolved.root }
      )
    );
    notifyOutcome(outcome, "workflow scaffold", channel);
    if (outcome.kind !== "ok" && outcome.kind !== "verdict") {
      return;
    }
    const document = outcome.document as Record<string, unknown> | undefined;
    const mission =
      document && typeof document["mission"] === "object" && document["mission"] !== null
        ? (document["mission"] as Record<string, unknown>)
        : undefined;
    const steps = mission && Array.isArray(mission["steps"]) ? mission["steps"] : undefined;
    if (steps) {
      await openUntitledJson({ steps });
      void vscode.window.showInformationMessage(
        "AURORA Agent: scaffolded steps opened as an untitled document — review, edit, and save it, then run 'Workflow: Instantiate Group'. " +
          "The scaffold ran a no-dispatch preflight only; nothing was executed."
      );
    } else {
      await summaries.openDocumentSummary(`Scaffold ${group.id}`, outcome.document, "generic");
      void vscode.window.showWarningMessage(
        "AURORA Agent: the scaffold document carried no mission.steps array; the full document is shown instead."
      );
    }
  });

  register("auroraAgent.instantiateWorkflow", async (item) => {
    const group = await pickWorkflowGroup(deps, item as WorkflowItem | undefined);
    if (!group) {
      return;
    }
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const inputs = await askMissionInputs();
    if (!inputs) {
      return;
    }
    const steps = await pickJsonFile("Select steps JSON", auroraDir());
    if (!steps) {
      return;
    }
    const outcome = await runWithProgress(`AURORA Agent: instantiating ${group.id}`, () =>
      runCli(
        resolved.bioprism,
        [
          "workflow",
          "instantiate",
          "--workflow",
          group.id,
          "--mission-id",
          inputs.missionId,
          "--goal",
          inputs.goal,
          "--steps",
          steps,
        ],
        channel,
        { cwd: resolved.root }
      )
    );
    notifyOutcome(outcome, "workflow instantiate", channel);
    if ((outcome.kind === "ok" || outcome.kind === "verdict") && outcome.document !== undefined) {
      await openUntitledJson(outcome.document);
      void vscode.window.showInformationMessage(
        "AURORA Agent: instantiation document opened as an untitled document — save it and pass it to 'Autopilot: Run' via --instantiation. " +
          "Instantiate ran an authoritative no-dispatch preflight; nothing was executed."
      );
    }
  });

  register("auroraAgent.createGrant", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const outcome = await runCli(resolved.bioprism, ["autopilot", "grant-template"], channel, { cwd: resolved.root });
    notifyOutcome(outcome, "autopilot grant-template", channel);
    if (outcome.kind === "ok" && outcome.document !== undefined) {
      await openUntitledJson(outcome.document);
      void vscode.window.showInformationMessage(
        "AURORA Agent: authority for autonomous dispatch comes only from this grant document — there is no default grant. " +
          "Edit allowed_tools and max_attempts, save the file, then use 'Autopilot: Run'."
      );
    }
  });

  const runAutopilot = async (dryRun: boolean) => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const instantiation = await pickJsonFile("Select instantiation JSON", auroraDir());
    if (!instantiation) {
      return;
    }
    const grant = await pickJsonFile("Select grant JSON", path.dirname(instantiation));
    if (!grant) {
      return;
    }
    let reportOut: string | undefined;
    if (!dryRun) {
      const reportsDir = reportsDirectory();
      if (!reportsDir) {
        void vscode.window.showErrorMessage(
          "AURORA Agent: open a folder or set auroraAgent.reportsDir so the report can be written."
        );
        return;
      }
      fs.mkdirSync(reportsDir, { recursive: true });
      reportOut = path.join(reportsDir, `report-${timestamp()}.json`);
    }

    if (!dryRun) {
      let grantText: string;
      try {
        grantText = fs.readFileSync(grant, "utf8");
      } catch (error) {
        void vscode.window.showErrorMessage(
          `AURORA Agent: could not read the grant file: ${error instanceof Error ? error.message : String(error)}`
        );
        return;
      }
      const summary = summarizeGrant(grantText);
      const confirmation = await vscode.window.showWarningMessage(
        "Autopilot will REALLY dispatch tools under this grant.",
        {
          modal: true,
          detail: grantConfirmationText(summary) + "\n\nAuthority comes only from the grant document itself.",
        },
        "Run Autopilot"
      );
      if (confirmation !== "Run Autopilot") {
        return;
      }
    }

    const args = ["autopilot", "run", "--instantiation", instantiation, "--grant", grant];
    if (dryRun) {
      args.push("--dry-run");
    } else if (reportOut) {
      args.push("--report-out", reportOut);
    }
    const outcome = await runWithProgress(
      dryRun ? "AURORA Agent: autopilot dry-run (no dispatch, zero writes)" : "AURORA Agent: autopilot running",
      () => runCli(resolved.bioprism, args, channel, { cwd: resolved.root })
    );
    notifyOutcome(outcome, dryRun ? "autopilot dry-run" : "autopilot run", channel);
    deps.reports.refresh();
    if (!dryRun && reportOut && fs.existsSync(reportOut)) {
      await summaries.openFileSummary(reportOut, "report");
    } else if (outcome.document !== undefined) {
      await summaries.openDocumentSummary(
        dryRun ? "Autopilot dry-run outcome" : "Autopilot outcome",
        outcome.document,
        documentLooksLikeReport(outcome.document) ? "report" : "generic"
      );
    }
  };

  register("auroraAgent.runAutopilot", () => runAutopilot(false));
  register("auroraAgent.dryRunAutopilot", () => runAutopilot(true));

  register("auroraAgent.verifyReport", async () => {
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    const report = await pickJsonFile("Select report JSON", reportsDirectory());
    if (!report) {
      return;
    }
    const outcome = await runWithProgress("AURORA Agent: verifying autopilot report", () =>
      runCli(resolved.bioprism, ["autopilot", "verify", "--report", report], channel, { cwd: resolved.root })
    );
    notifyOutcome(outcome, "autopilot verify", channel);
    if (outcome.document !== undefined) {
      await summaries.openDocumentSummary("Autopilot report verification", outcome.document, "generic");
    }
  });

  let gatewayExecution: vscode.TaskExecution | undefined;
  context.subscriptions.push(
    vscode.tasks.onDidEndTask((event) => {
      if (event.execution === gatewayExecution) {
        gatewayExecution = undefined;
      }
    })
  );

  register("auroraAgent.gatewayStart", async () => {
    if (gatewayExecution) {
      void vscode.window.showInformationMessage("AURORA Agent: the gateway task is already running.");
      return;
    }
    const resolved = await requireBinary(deps);
    if (!resolved) {
      return;
    }
    if (!fs.existsSync(resolved.bioprismApi)) {
      void vscode.window.showErrorMessage(`AURORA Agent: bioprism-api not found at ${resolved.bioprismApi}.`);
      return;
    }
    const bind = "127.0.0.1:8787";
    const args = ["--bind", bind];
    if (resolved.root) {
      args.push("--root", resolved.root);
    }
    channel.appendLine(`$ ${resolved.bioprismApi} ${args.join(" ")} (as task; loopback bind, no --token supplied)`);
    const task = new vscode.Task(
      { type: "auroraAgent.gateway" },
      vscode.TaskScope.Workspace,
      "bioprism-api",
      "AURORA Agent",
      new vscode.ProcessExecution(resolved.bioprismApi, args)
    );
    task.isBackground = true;
    gatewayExecution = await vscode.tasks.executeTask(task);
    void vscode.window.showInformationMessage(`AURORA Agent: gateway starting on ${bind} (terminal task 'bioprism-api').`);
  });

  register("auroraAgent.gatewayStop", () => {
    const running =
      gatewayExecution ??
      vscode.tasks.taskExecutions.find((execution) => execution.task.name === "bioprism-api");
    if (!running) {
      void vscode.window.showInformationMessage("AURORA Agent: no gateway task is running.");
      return;
    }
    running.terminate();
    gatewayExecution = undefined;
    void vscode.window.showInformationMessage("AURORA Agent: gateway task terminated.");
  });

  register("auroraAgent.doctor", async () => {
    channel.appendLine("");
    channel.appendLine("=== AURORA Agent doctor ===");
    for (const line of deps.resolver.describeResolution()) {
      channel.appendLine(line);
    }
    const resolved = deps.resolver.resolve();
    if (resolved) {
      const version = await runPlain(resolved.bioprism, ["--version"], channel);
      channel.appendLine(
        version.exitCode === 0
          ? `bioprism --version: ${version.stdout.trim()}`
          : `bioprism --version failed (exit ${version.exitCode}): ${version.stderr.trim()}`
      );
    }
    channel.appendLine(
      `MCP server definition provider API: ${deps.mcpAvailable ? "available (registered as 'AURORA Agent')" : "NOT available in this VS Code build"}`
    );
    channel.appendLine(describeMcpTarget(deps.resolver));
    const gh = await runPlain("gh", ["--version"], channel);
    channel.appendLine(
      gh.exitCode === 0
        ? `gh: ${gh.stdout.split(/\r?\n/)[0]}`
        : "gh: not found on PATH (Pipelines view will show an informational row)"
    );
    const reportsDir = reportsDirectory();
    channel.appendLine(`reports directory: ${reportsDir ?? "(no workspace folder and no auroraAgent.reportsDir)"}`);
    channel.appendLine("=== end doctor ===");
    channel.show(true);
    deps.onBinaryStateMaybeChanged();
  });

  register("auroraAgent.copyMcpJson", () => copyMcpJson(deps.resolver));

  register("auroraAgent.openReport", async (filePath) => {
    if (typeof filePath === "string") {
      await summaries.openFileSummary(filePath, "report");
    }
  });

  register("auroraAgent.openRun", async (url) => {
    if (typeof url === "string" && /^https:\/\//.test(url)) {
      await vscode.env.openExternal(vscode.Uri.parse(url));
    }
  });

  register("auroraAgent.refreshWorkflows", () => deps.workflows.refresh());
  register("auroraAgent.refreshReports", () => deps.reports.refresh());
  register("auroraAgent.refreshPipelines", () => deps.pipelines.refresh());
}
