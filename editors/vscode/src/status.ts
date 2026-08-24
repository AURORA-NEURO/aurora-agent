import * as vscode from "vscode";
import * as path from "path";
import { Resolver } from "./resolve";

export class StatusBar implements vscode.Disposable {
  private readonly item: vscode.StatusBarItem;

  constructor(private readonly resolver: Resolver) {
    this.item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 50);
    this.item.command = "auroraAgent.doctor";
    this.update();
    this.item.show();
  }

  update(): void {
    const resolved = this.resolver.resolve();
    if (resolved) {
      const rootName = resolved.root ? path.basename(resolved.root) : "no root";
      this.item.text = `$(beaker) AURORA: ${rootName}`;
      this.item.tooltip = [
        `bioprism: ${resolved.bioprism}`,
        `binary source: ${resolved.binarySource}`,
        `root: ${resolved.root ?? "(none)"} (${resolved.rootSource})`,
        "Click for doctor.",
      ].join("\n");
      this.item.backgroundColor = undefined;
    } else {
      this.item.text = "$(warning) AURORA: no binary";
      this.item.tooltip = "No bioprism binary found. Click for doctor and resolution details.";
      this.item.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
    }
  }

  dispose(): void {
    this.item.dispose();
  }
}
