import * as vscode from "vscode";
import { spawn } from "child_process";
import { CliOutcome, parseEnvelope, describeOutcome } from "./envelope";

export interface RunOptions {
  cwd?: string;
  timeoutMs?: number;
}

export function runCli(
  exe: string,
  args: string[],
  channel: vscode.OutputChannel,
  options: RunOptions = {}
): Promise<CliOutcome> {
  const fullArgs = ["--json", ...args];
  channel.appendLine(`$ ${exe} ${fullArgs.join(" ")}`);
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(exe, fullArgs, { cwd: options.cwd, windowsHide: true });
    } catch (error) {
      resolve({
        kind: "crash",
        exitCode: null,
        document: undefined,
        message: `failed to spawn ${exe}: ${error instanceof Error ? error.message : String(error)}`,
        rawStdout: "",
        stderr: "",
      });
      return;
    }
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    let settled = false;
    const settle = (outcome: CliOutcome) => {
      if (!settled) {
        settled = true;
        if (outcome.kind !== "ok") {
          channel.appendLine(`  -> ${describeOutcome(outcome)}`);
        } else {
          channel.appendLine(`  -> exit 0`);
        }
        resolve(outcome);
      }
    };
    const timeout = options.timeoutMs
      ? setTimeout(() => {
          child.kill();
          settle({
            kind: "crash",
            exitCode: null,
            document: undefined,
            message: `timed out after ${options.timeoutMs} ms`,
            rawStdout: Buffer.concat(stdout).toString("utf8"),
            stderr: Buffer.concat(stderr).toString("utf8"),
          });
        }, options.timeoutMs)
      : undefined;
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", (error) => {
      if (timeout) {
        clearTimeout(timeout);
      }
      settle({
        kind: "crash",
        exitCode: null,
        document: undefined,
        message: `failed to run ${exe}: ${error.message}`,
        rawStdout: "",
        stderr: "",
      });
    });
    child.on("close", (code) => {
      if (timeout) {
        clearTimeout(timeout);
      }
      settle(parseEnvelope(code, Buffer.concat(stdout).toString("utf8"), Buffer.concat(stderr).toString("utf8")));
    });
  });
}

export function runPlain(
  exe: string,
  args: string[],
  channel: vscode.OutputChannel,
  options: RunOptions = {}
): Promise<{ exitCode: number | null; stdout: string; stderr: string }> {
  channel.appendLine(`$ ${exe} ${args.join(" ")}`);
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(exe, args, { cwd: options.cwd, windowsHide: true });
    } catch (error) {
      resolve({ exitCode: null, stdout: "", stderr: error instanceof Error ? error.message : String(error) });
      return;
    }
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", (error) => resolve({ exitCode: null, stdout: "", stderr: error.message }));
    child.on("close", (code) =>
      resolve({
        exitCode: code,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      })
    );
  });
}

export function notifyOutcome(outcome: CliOutcome, action: string, channel: vscode.OutputChannel): void {
  const show = "Show Output";
  const handle = (choice: string | undefined) => {
    if (choice === show) {
      channel.show(true);
    }
  };
  switch (outcome.kind) {
    case "ok":
      break;
    case "verdict":
      void vscode.window
        .showWarningMessage(`AURORA Agent ${action} — ${describeOutcome(outcome)}`, show)
        .then(handle);
      break;
    case "failure":
      void vscode.window
        .showErrorMessage(`AURORA Agent ${action} failed — ${describeOutcome(outcome)}`, show)
        .then(handle);
      break;
    case "crash":
      void vscode.window
        .showErrorMessage(`AURORA Agent ${action}: ${describeOutcome(outcome)}`, show)
        .then(handle);
      break;
  }
}
