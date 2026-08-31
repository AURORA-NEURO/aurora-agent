import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { renderCertificateSummary, renderReportSummary, renderGenericSummary } from "./summaries";

export const SCHEME = "aurora-agent";

export type SummaryKind = "certificate" | "report" | "generic";

const MAX_ENTRIES = 50;

export class SummaryProvider implements vscode.TextDocumentContentProvider {
  private readonly contents = new Map<string, string>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri>();
  private counter = 0;

  readonly onDidChange = this.changed.event;

  provideTextDocumentContent(uri: vscode.Uri): string {
    return this.contents.get(uri.toString()) ?? "_This summary is no longer available; re-run the command that produced it._";
  }

  register(context: vscode.ExtensionContext): void {
    context.subscriptions.push(vscode.workspace.registerTextDocumentContentProvider(SCHEME, this));
    context.subscriptions.push(this.changed);
  }

  private nextId(): number {
    this.counter += 1;
    return this.counter;
  }

  private storeContent(uriPath: string, content: string): vscode.Uri {
    const uri = vscode.Uri.from({ scheme: SCHEME, path: uriPath });
    const key = uri.toString();
    this.contents.delete(key);
    this.contents.set(key, content);
    while (this.contents.size > MAX_ENTRIES) {
      const oldest = this.contents.keys().next().value;
      if (oldest === undefined) {
        break;
      }
      this.contents.delete(oldest);
    }
    this.changed.fire(uri);
    return uri;
  }

  async openFileSummary(filePath: string, kind: SummaryKind): Promise<void> {
    let document: unknown;
    try {
      document = JSON.parse(fs.readFileSync(filePath, "utf8"));
    } catch (error) {
      void vscode.window.showErrorMessage(
        `AURORA Agent: could not read ${filePath} as JSON: ${error instanceof Error ? error.message : String(error)}`
      );
      return;
    }
    const rawLink = vscode.Uri.file(filePath).toString();
    const name = path.basename(filePath, ".json");
    await this.openRendered(name, document, kind, rawLink);
  }

  async openDocumentSummary(name: string, document: unknown, kind: SummaryKind): Promise<void> {
    await this.openRendered(name, document, kind, undefined);
  }

  private async openRendered(
    name: string,
    document: unknown,
    kind: SummaryKind,
    rawLink: string | undefined
  ): Promise<void> {
    const id = this.nextId();
    const raw =
      rawLink ??
      this.storeContent(`/summary/${id}/${name}-raw.json`, JSON.stringify(document, null, 2) + "\n").toString();
    let markdown: string;
    switch (kind) {
      case "certificate":
        markdown = renderCertificateSummary(document, raw);
        break;
      case "report":
        markdown = renderReportSummary(document, raw);
        break;
      case "generic":
        markdown = renderGenericSummary(name, document, raw);
        break;
    }
    const uri = this.storeContent(`/summary/${id}/${name}.md`, markdown);
    const doc = await vscode.workspace.openTextDocument(uri);
    try {
      await vscode.commands.executeCommand("markdown.showPreview", uri);
    } catch {
      await vscode.window.showTextDocument(doc, { preview: true });
    }
  }
}
