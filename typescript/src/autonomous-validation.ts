/** Shared bounded scalar validation for autonomous contracts. */

import { ArgumentError, isObject } from "./errors.js";

export function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

export function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

export function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

export function boundedModelDigest(name: string, value: unknown): string {
  const digest = boundedText(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return digest;
}

export function assertSafeTransientValue(value: unknown, depth = 0): void {
  if (depth > 32) throw new ArgumentError("autonomous transient context is too deeply nested");
  if (Array.isArray(value)) { for (const child of value) assertSafeTransientValue(child, depth + 1); return; }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "password", "secret", "token", "privatekey", "refreshtoken"].includes(normalized)) throw new ArgumentError("autonomous transient context cannot contain credential-shaped fields");
      assertSafeTransientValue(child, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError("autonomous transient context contains a non-finite number");
}
