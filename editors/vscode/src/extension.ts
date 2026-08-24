import * as vscode from "vscode";
import { createResolver } from "./resolve";
import { registerMcpProvider } from "./mcp";
import { SummaryProvider } from "./render";
import { WorkflowsProvider, ReportsProvider, PipelinesProvider } from "./views";
import { registerCommands } from "./commands";
import { StatusBar } from "./status";

export function activate(context: vscode.ExtensionContext): void {
  const channel = vscode.window.createOutputChannel("AURORA Agent");
  context.subscriptions.push(channel);
  const log = (line: string) => channel.appendLine(line);
  log(`AURORA Agent extension activated (${new Date().toISOString()})`);

  const resolver = createResolver(context, log);

  const summaries = new SummaryProvider();
  summaries.register(context);

  const workflows = new WorkflowsProvider(resolver, channel);
  context.subscriptions.push(vscode.window.registerTreeDataProvider("auroraAgent.workflows", workflows));

  const reports = new ReportsProvider();
  context.subscriptions.push(reports);
  context.subscriptions.push(vscode.window.registerTreeDataProvider("auroraAgent.reports", reports));

  const pipelines = new PipelinesProvider(channel);
  context.subscriptions.push(vscode.window.registerTreeDataProvider("auroraAgent.pipelines", pipelines));

  const mcpAvailable = registerMcpProvider(context, resolver, log);

  const statusBar = new StatusBar(resolver);
  context.subscriptions.push(statusBar);

  registerCommands({
    context,
    resolver,
    channel,
    summaries,
    workflows,
    reports,
    pipelines,
    mcpAvailable,
    onBinaryStateMaybeChanged: () => statusBar.update(),
  });

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("auroraAgent")) {
        statusBar.update();
        reports.refresh();
        workflows.refresh();
      }
    })
  );

  const initial = resolver.resolve();
  log(
    initial
      ? `resolved bioprism at ${initial.bioprism} (source: ${initial.binarySource}; root: ${initial.root ?? "none"} via ${initial.rootSource})`
      : "no bioprism binary resolved yet; commands will offer the managed download"
  );
}

export function deactivate(): void {}
