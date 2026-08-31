import * as vscode from "vscode";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { createHash } from "crypto";
import { spawnSync } from "child_process";
import { RELEASE_VERSION, RELEASE_BASE_URL, platformArchive, pinnedSha256 } from "./pins";
import { resolveBinaries, ResolvedBinaries } from "./resolveCore";

export type { ResolvedBinaries } from "./resolveCore";

export interface Resolver {
  resolve(): ResolvedBinaries | undefined;
  resolveOrOfferDownload(): Promise<ResolvedBinaries | undefined>;
  cacheVersionDir(): string;
  describeResolution(): string[];
}

function config() {
  return vscode.workspace.getConfiguration("auroraAgent");
}

function settingString(key: string): string | undefined {
  const value = config().get<string>(key);
  return value && value.trim() !== "" ? value.trim() : undefined;
}

export function createResolver(context: vscode.ExtensionContext, log: (line: string) => void): Resolver {
  const cacheVersionDir = () =>
    path.join(context.globalStorageUri.fsPath, "aurora-agent", `v${RELEASE_VERSION}`);

  const inputs = () => ({
    platform: process.platform as string,
    settingsBinaryDir: settingString("binaryDir"),
    settingsRoot: settingString("root"),
    envRoot: process.env["AURORA_AGENT_ROOT"],
    workspaceFolders: (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath),
    homeDir: os.homedir(),
    cacheVersionDir: cacheVersionDir(),
    existsSync: (candidate: string) => fs.existsSync(candidate),
    readdirSync: (dir: string) => fs.readdirSync(dir),
    isDirectory: (candidate: string) => {
      try {
        return fs.statSync(candidate).isDirectory();
      } catch {
        return false;
      }
    },
  });

  const resolve = () => resolveBinaries(inputs());

  const resolveOrOfferDownload = async (): Promise<ResolvedBinaries | undefined> => {
    const found = resolve();
    if (found) {
      return found;
    }
    const archive = platformArchive(process.platform, process.arch);
    if (archive === undefined) {
      void vscode.window.showErrorMessage(
        `AURORA Agent: no bioprism binary found, and no prebuilt v${RELEASE_VERSION} archive exists for ${process.platform}/${process.arch}. ` +
          "Build from source (cargo build --release) and set auroraAgent.binaryDir or AURORA_AGENT_ROOT."
      );
      return undefined;
    }
    const choice = await vscode.window.showInformationMessage(
      `AURORA Agent: no bioprism binary found in settings, AURORA_AGENT_ROOT, workspace folders, ~/aurora-agent, ~/bioprism, or the download cache. ` +
        `Download ${archive} from GitHub Releases (SHA-256 verified against a hash pinned in the extension)?`,
      { modal: true },
      "Download"
    );
    if (choice !== "Download") {
      return undefined;
    }
    try {
      await downloadAndCache(cacheVersionDir(), log);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`AURORA Agent download failed: ${message}`);
      return undefined;
    }
    const after = resolve();
    if (!after) {
      void vscode.window.showErrorMessage(
        "AURORA Agent: the downloaded archive did not contain a bioprism binary in an expected layout. " +
          `Inspect ${cacheVersionDir()} or point auroraAgent.binaryDir at a build.`
      );
    }
    return after;
  };

  const describeResolution = (): string[] => {
    const current = inputs();
    const lines: string[] = [];
    lines.push(`settings auroraAgent.binaryDir: ${current.settingsBinaryDir ?? "(not set)"}`);
    lines.push(`env AURORA_AGENT_ROOT: ${current.envRoot ?? "(not set)"}`);
    lines.push(
      `workspace folders: ${current.workspaceFolders.length > 0 ? current.workspaceFolders.join(", ") : "(none)"}`
    );
    lines.push(`home candidates: ${path.join(current.homeDir, "aurora-agent")}, ${path.join(current.homeDir, "bioprism")}`);
    lines.push(`download cache: ${current.cacheVersionDir}`);
    const found = resolve();
    if (found) {
      lines.push(`resolved bioprism: ${found.bioprism} (source: ${found.binarySource})`);
      lines.push(`bioprism-mcp: ${found.bioprismMcp}${fs.existsSync(found.bioprismMcp) ? "" : " (MISSING)"}`);
      lines.push(`bioprism-api: ${found.bioprismApi}${fs.existsSync(found.bioprismApi) ? "" : " (MISSING)"}`);
      lines.push(`root: ${found.root ?? "(none)"} (source: ${found.rootSource})`);
      if (found.rootSource === "bundle") {
        lines.push(
          "note: the managed download ships binaries only — its default root has no fixtures. " +
            "Point auroraAgent.root at a checkout for fixture-dependent features."
        );
      }
    } else {
      lines.push("resolved: nothing — no bioprism binary found in any source");
    }
    return lines;
  };

  return { resolve, resolveOrOfferDownload, cacheVersionDir, describeResolution };
}

async function downloadFollowingRedirects(url: string): Promise<Buffer> {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`download failed: HTTP ${response.status} for ${url}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

function extractZipWithPowerShell(archivePath: string, destDir: string): void {
  const quote = (value: string) => `'${value.replace(/'/g, "''")}'`;
  const command = `Expand-Archive -LiteralPath ${quote(archivePath)} -DestinationPath ${quote(destDir)} -Force`;
  const result = spawnSync("powershell", ["-NoProfile", "-Command", command], {
    stdio: ["ignore", "ignore", "pipe"],
  });
  if (result.error || result.status !== 0) {
    const detail = result.error
      ? result.error.message
      : `exit code ${result.status}${result.stderr ? `: ${result.stderr.toString().trim()}` : ""}`;
    throw new Error(
      `could not extract ${path.basename(archivePath)}: System32\\tar.exe is missing and PowerShell Expand-Archive failed (${detail}). ` +
        "Extract the archive manually and set auroraAgent.binaryDir to the directory containing the binaries."
    );
  }
}

function extractArchive(archivePath: string, destDir: string): void {
  fs.mkdirSync(destDir, { recursive: true });
  const isZip = archivePath.endsWith(".zip");
  const sysTar = path.join(process.env["SystemRoot"] ?? "C:\\Windows", "System32", "tar.exe");
  if (process.platform === "win32" && isZip && !fs.existsSync(sysTar)) {
    extractZipWithPowerShell(archivePath, destDir);
    return;
  }
  const tarCmd = process.platform === "win32" && fs.existsSync(sysTar) ? sysTar : "tar";
  const args = isZip ? ["-xf", archivePath, "-C", destDir] : ["-xzf", archivePath, "-C", destDir];
  const result = spawnSync(tarCmd, args, { stdio: ["ignore", "ignore", "pipe"] });
  if (result.error) {
    throw new Error(`could not run ${tarCmd}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = result.stderr ? result.stderr.toString() : "";
    throw new Error(`${tarCmd} exited with code ${result.status}: ${stderr.trim()}`);
  }
}

export async function downloadAndCache(cacheVersionDir: string, log: (line: string) => void): Promise<void> {
  const archiveName = platformArchive(process.platform, process.arch);
  if (archiveName === undefined) {
    throw new Error(`no prebuilt archive for ${process.platform}/${process.arch}`);
  }
  const expected = pinnedSha256(archiveName);
  if (expected === undefined) {
    throw new Error(`no pinned SHA-256 for ${archiveName}; refusing to download`);
  }
  const url = RELEASE_BASE_URL + archiveName;

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: `AURORA Agent: downloading ${archiveName}` },
    async (progress) => {
      log(`downloading ${url}`);
      const bytes = await downloadFollowingRedirects(url);
      progress.report({ message: "verifying SHA-256" });

      const digest = createHash("sha256").update(bytes).digest("hex");
      if (digest !== expected) {
        throw new Error(
          `SHA-256 mismatch for ${archiveName}: got ${digest}, expected ${expected}; the download was discarded and nothing was extracted`
        );
      }
      log(`sha256 verified: ${digest}`);

      fs.mkdirSync(path.dirname(cacheVersionDir), { recursive: true });
      const archivePath = path.join(path.dirname(cacheVersionDir), archiveName);
      fs.writeFileSync(archivePath, bytes);

      progress.report({ message: "extracting" });
      const partial = `${cacheVersionDir}.partial`;
      fs.rmSync(partial, { recursive: true, force: true });
      try {
        extractArchive(archivePath, partial);
      } finally {
        fs.rmSync(archivePath, { force: true });
      }
      fs.rmSync(cacheVersionDir, { recursive: true, force: true });
      fs.renameSync(partial, cacheVersionDir);
      log(`extracted to ${cacheVersionDir}`);
    }
  );
}
