export type Retryability = "terminal" | "retryable_after_change" | "retryable_as_is";

export type OutcomeKind = "ok" | "verdict" | "failure" | "crash";

export interface CliOutcome {
  kind: OutcomeKind;
  exitCode: number | null;
  document: unknown;
  message: string;
  errorKind?: string;
  retryability?: Retryability;
  rawStdout: string;
  stderr: string;
}

export const EXIT_MEANINGS: Record<number, string> = {
  0: "the command completed and its assertion held",
  1: "completed, but the checked property does not hold",
  2: "bad invocation",
  3: "input failed its schema or could not be parsed",
  4: "no result satisfies the declared contract",
  5: "a declared dependency could not be read or written",
  6: "contradicts state already committed under this id",
  7: "policy refused; the platform behaved correctly",
  8: "ran correctly; the evidence does not decide",
  9: "a precondition was superseded; re-read and re-send",
};

export function retryabilityForExit(code: number): Retryability | undefined {
  switch (code) {
    case 2:
    case 3:
    case 6:
      return "terminal";
    case 4:
    case 7:
    case 8:
      return "retryable_after_change";
    case 5:
    case 9:
      return "retryable_as_is";
    default:
      return undefined;
  }
}

function extractSingleJson(stdout: string): unknown | undefined {
  const trimmed = stdout.trim();
  if (trimmed === "") {
    return undefined;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return undefined;
  }
}

function asRetryability(value: unknown): Retryability | undefined {
  if (value === "terminal" || value === "retryable_after_change" || value === "retryable_as_is") {
    return value;
  }
  return undefined;
}

export function parseEnvelope(exitCode: number | null, stdout: string, stderr: string): CliOutcome {
  if (exitCode === null) {
    return {
      kind: "crash",
      exitCode,
      document: undefined,
      message: stderr.trim() || "the process was terminated by a signal before it produced a result",
      rawStdout: stdout,
      stderr,
    };
  }

  const document = extractSingleJson(stdout);

  if (exitCode === 0) {
    if (document === undefined) {
      return {
        kind: "crash",
        exitCode,
        document: undefined,
        message: "exit 0 but stdout was not a single JSON document",
        rawStdout: stdout,
        stderr,
      };
    }
    return {
      kind: "ok",
      exitCode,
      document,
      message: EXIT_MEANINGS[0],
      rawStdout: stdout,
      stderr,
    };
  }

  if (exitCode === 1) {
    return {
      kind: "verdict",
      exitCode,
      document,
      message: EXIT_MEANINGS[1],
      rawStdout: stdout,
      stderr,
    };
  }

  let errorKind: string | undefined;
  let retryability: Retryability | undefined;
  let message: string | undefined;
  if (document && typeof document === "object") {
    const error = (document as Record<string, unknown>)["error"];
    if (error && typeof error === "object") {
      const record = error as Record<string, unknown>;
      if (typeof record["kind"] === "string") {
        errorKind = record["kind"];
      }
      retryability = asRetryability(record["retryability"]);
      if (typeof record["message"] === "string") {
        message = record["message"];
        if (typeof record["subject"] === "string" && record["subject"] !== "") {
          message = `${record["subject"]}: ${message}`;
        }
      }
    }
  }
  if (retryability === undefined) {
    retryability = retryabilityForExit(exitCode);
  }
  if (message === undefined) {
    message = stderr.trim() || EXIT_MEANINGS[exitCode] || `bioprism exited with code ${exitCode}`;
  }
  return {
    kind: "failure",
    exitCode,
    document,
    message,
    errorKind,
    retryability,
    rawStdout: stdout,
    stderr,
  };
}

export function describeOutcome(outcome: CliOutcome): string {
  switch (outcome.kind) {
    case "ok":
      return outcome.message;
    case "verdict":
      return `verdict (exit 1): ${outcome.message}`;
    case "failure": {
      const kind = outcome.errorKind ? `[${outcome.errorKind}] ` : "";
      const retry = outcome.retryability ? ` (retryability: ${outcome.retryability})` : "";
      return `${kind}${outcome.message}${retry}`;
    }
    case "crash":
      return `bioprism did not produce a result: ${outcome.message}`;
  }
}
