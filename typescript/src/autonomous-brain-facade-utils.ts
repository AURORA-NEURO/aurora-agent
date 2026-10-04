import { ArgumentError, ProviderRuntimeError } from "./errors.js";

function textBytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || textBytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

export function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

export function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

export function errorProjection(error: unknown): { error_class: string; failure_code: string } {
  if (error instanceof ProviderRuntimeError) return { error_class: error.constructor.name, failure_code: error.code };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { error_class: error.constructor.name, failure_code: "error" };
  return { error_class: "AutonomousBrainError", failure_code: "error" };
}
