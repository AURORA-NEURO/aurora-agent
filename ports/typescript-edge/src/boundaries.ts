/**
 * Boundary type shapes for MCP, REST and webhook surfaces.
 *
 * This module is types only: it compiles to zero runtime statements, on purpose. A shape can
 * describe what crosses a boundary without implementing the transport that carries it, and
 * keeping the module type-only makes "no live client lives here" verifiable rather than
 * asserted - test/boundaries.test.mjs checks the emitted JavaScript is empty.
 *
 * Deliberately absent by design, not by omission: transport clients, fetch/socket usage,
 * authentication or OAuth fields, provider names, retry schedulers, and credential material.
 * Header maps expose header NAMES as data; values would be where secrets leak in.
 */

/** Correlation identifier shared by all three boundary shapes; branded against bare strings. */
declare const correlationBrand: unique symbol;
export type CorrelationId = string & { readonly [correlationBrand]: "CorrelationId" };

/** MCP JSON-RPC 2.0 envelope shapes (spec-structural subset; no method vocabulary claimed). */
export declare namespace Mcp {
  export interface Request<Params = unknown> {
    readonly jsonrpc: "2.0";
    readonly id: string | number;
    readonly method: string;
    readonly params?: Params;
  }

  export interface Notification<Params = unknown> {
    readonly jsonrpc: "2.0";
    readonly method: string;
    readonly params?: Params;
  }

  export interface RpcErrorObject {
    readonly code: number;
    readonly message: string;
    readonly data?: unknown;
  }

  export type Response<Result = unknown> =
    | { readonly jsonrpc: "2.0"; readonly id: string | number; readonly result: Result }
    | { readonly jsonrpc: "2.0"; readonly id: string | number; readonly error: RpcErrorObject };

  /** Tool metadata as declared over MCP; `inputSchema` is carried opaquely, never evaluated. */
  export interface ToolDescriptor {
    readonly name: string;
    readonly title?: string;
    readonly description?: string;
    readonly inputSchema: unknown;
  }
}

/** REST request/response shapes: structure of a call, never its execution. */
export declare namespace Rest {
  export type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

  export interface RequestShape {
    readonly correlationId: CorrelationId;
    readonly method: Method;
    /** Path template such as `/v1/tasks/{task_id}`; parameters are named, not resolved. */
    readonly pathTemplate: string;
    readonly queryParameterNames: readonly string[];
    /** Names only. Header values are transport payload and stay outside this slice. */
    readonly headerNames: readonly string[];
    readonly contentType?: string;
    readonly bodyByteLength?: number;
  }

  export interface ResponseShape {
    readonly correlationId: CorrelationId;
    readonly statusCode: number;
    readonly contentType?: string;
    readonly bodyByteLength?: number;
  }
}

/** Webhook delivery shapes: what an event envelope and its acknowledgement look like. */
export declare namespace Webhook {
  export interface EventEnvelopeShape {
    readonly eventId: string;
    readonly eventType: string;
    /** RFC 3339 timestamp string; parsed time semantics belong to the consumer. */
    readonly occurredAt: string;
    readonly payloadByteLength: number;
    /** Delivery signature HEADER NAME only; no scheme, key, or signature value is modeled. */
    readonly signatureHeaderName?: string;
  }

  export interface AcknowledgementShape {
    readonly eventId: string;
    readonly accepted: boolean;
    readonly reason?: string;
  }
}
