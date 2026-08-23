import { ApiClient } from "./client.js";
import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import type { DomainToolExecutor } from "./autonomous.js";
import { ToolCatalogue } from "./tooling.js";
import type { ClientRequestOptions, JsonValue } from "./types.js";

/** Identifies the transport bridge without implying that transport success is task success. */
export const AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA = "bioprism-typescript-autonomous-api-tool-adapter/0.1" as const;

export interface AutonomousApiToolExecutorOptions {
  /** The exact catalogue already reviewed by the domain registry. */
  catalogue: ToolCatalogue;
  /** Optional caller-owned request headers, abort signal, and request correlation id. */
  requestOptions?: ClientRequestOptions;
}

function boundedErrorDetail(value: unknown): string {
  const text = value instanceof Error ? value.constructor.name : typeof value === "string" ? value : "tool transport refusal";
  return text.slice(0, 256);
}

function toolPayload(response: Awaited<ReturnType<ApiClient["toolChecked"]>>): JsonValue {
  const result = response.mcp.result;
  if (!result) throw new ProviderRuntimeError("API tool response omitted its MCP result");
  if (result.structuredContent !== undefined) return result.structuredContent;
  if (result.content !== undefined) return result.content as unknown as JsonValue;
  return result as unknown as JsonValue;
}

/**
 * Build the default live-tool executor for an agent whose caller supplied an ApiClient and a
 * reviewed ToolCatalogue. The bridge performs no discovery and accepts no credential material;
 * ApiClient remains responsible for the user's configured transport/session and the domain
 * runtime remains responsible for stage admission, approval, effect journaling, and evidence.
 */
export function createAutonomousApiToolExecutor(
  client: ApiClient,
  options: AutonomousApiToolExecutorOptions,
): DomainToolExecutor {
  if (!(client instanceof ApiClient)) throw new ArgumentError("autonomous API tool adapter requires an ApiClient");
  if (!options || !(options.catalogue instanceof ToolCatalogue)) throw new ArgumentError("autonomous API tool adapter requires a ToolCatalogue");
  return async (tool, arguments_) => {
    try {
      const response = await client.toolChecked(tool.name, arguments_, options.requestOptions, options.catalogue);
      client.requireToolSuccess(response);
      return toolPayload(response);
    } catch (error) {
      // Preserve only a typed class at the autonomous receipt boundary. The raw response,
      // headers, and any server payload remain in the caller's transient transport scope.
      if (error instanceof ProviderRuntimeError) throw error;
      throw new ProviderRuntimeError(`API tool ${tool.name} was refused or failed: ${boundedErrorDetail(error)}`);
    }
  };
}
