/**
 * Streaming deterministic manifest metadata, transcribed from
 * integrations/scale-5m/aurora_scale/manifest.py.
 *
 * The implementation holds at most one bounded chunk of records at a time; a five-million-line
 * repository is therefore a scale target, not a requirement to hold five million records in
 * memory. Synthetic records model that size without creating millions of files - each record's
 * digest is explicitly a logical digest of its declared geometry, not of any file bytes.
 */

import { canonicalJsonString, type CanonicalValue } from "./canonical.js";
import { sha256Hex } from "./digest.js";

/** States what the synthetic benchmark measures: bound-model record/chunk state, never RSS. */
export const SYNTHETIC_MODEL_DESCRIPTION =
  "synthetic-logical-records; no files created";

export interface FileRecord {
  readonly path: string;
  readonly size: number;
  readonly lines: number;
  readonly digest: string;
}

export interface ManifestChunk {
  readonly index: number;
  readonly records: readonly FileRecord[];
  readonly digest: string;
  readonly bytes: number;
  readonly lines: number;
}

export interface ManifestSummary {
  readonly root: string;
  readonly total_files: number;
  readonly total_bytes: number;
  readonly total_lines: number;
  readonly chunk_count: number;
  readonly root_digest: string;
}

export class ManifestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ManifestError";
  }
}

const encoder = new TextEncoder();

/**
 * Yields logical records for scale tests; it never creates files or line-sized buffers.
 * Dimensions that make no sense throw rather than degrading to an empty stream.
 */
export async function* syntheticRecords(options: {
  fileCount: number;
  linesPerFile: number;
  bytesPerLine?: number;
}): AsyncGenerator<FileRecord> {
  const bytesPerLine = options.bytesPerLine ?? 80;
  if (
    !Number.isSafeInteger(options.fileCount) || options.fileCount < 0 ||
    !Number.isSafeInteger(options.linesPerFile) || options.linesPerFile < 0 ||
    !Number.isSafeInteger(bytesPerLine) || bytesPerLine <= 0
  ) {
    throw new ManifestError(
      "synthetic dimensions must be nonnegative with positive bytesPerLine",
    );
  }
  for (let index = 0; index < options.fileCount; index += 1) {
    const id = String(index).padStart(8, "0");
    const path = `synthetic/module-${id}.src`;
    const logical = `synthetic:${path}:${options.linesPerFile}:${bytesPerLine}`;
    yield {
      path,
      size: options.linesPerFile * bytesPerLine,
      lines: options.linesPerFile,
      digest: await sha256Hex(logical),
    };
  }
}

/**
 * The exact canonical bytes CPython's json.dumps(asdict(record), sort_keys=True,
 * separators=(",", ":"), ensure_ascii=False) + "\n" produces; sort_keys fixes the field order
 * to digest, lines, path, size.
 */
export function canonicalRecordLine(record: FileRecord): string {
  return `${canonicalJsonString(record as unknown as CanonicalValue)}\n`;
}

function recordBytes(record: FileRecord): Uint8Array {
  return encoder.encode(canonicalRecordLine(record));
}

/**
 * Batches a record stream into bounded chunks whose digests are stable across runs given the
 * same records: the chunk digest covers the exact canonical bytes of exactly its records.
 */
export async function* chunksFromRecords(
  records: AsyncIterable<FileRecord>,
  chunkRecords: number,
): AsyncGenerator<ManifestChunk> {
  if (!Number.isSafeInteger(chunkRecords) || chunkRecords <= 0) {
    throw new ManifestError("chunkRecords must be positive");
  }
  let index = 0;
  let bucket: FileRecord[] = [];
  let encodedLines: Uint8Array[] = [];
  let bufferedBytes = 0;
  for await (const record of records) {
    bucket.push(record);
    const line = recordBytes(record);
    encodedLines.push(line);
    bufferedBytes += line.length;
    if (bucket.length === chunkRecords) {
      yield await seal(index, bucket, encodedLines, bufferedBytes);
      index += 1;
      bucket = [];
      encodedLines = [];
      bufferedBytes = 0;
    }
  }
  if (bucket.length > 0) {
    yield await seal(index, bucket, encodedLines, bufferedBytes);
  }
}

async function seal(
  index: number,
  bucket: readonly FileRecord[],
  encodedLines: readonly Uint8Array[],
  total: number,
): Promise<ManifestChunk> {
  const concatenated = new Uint8Array(total);
  let offset = 0;
  for (const line of encodedLines) {
    concatenated.set(line, offset);
    offset += line.length;
  }
  let bytes = 0;
  let lines = 0;
  for (const record of bucket) {
    bytes += record.size;
    lines += record.lines;
  }
  return {
    index,
    records: bucket,
    digest: await sha256Hex(concatenated),
    bytes,
    lines,
  };
}

/**
 * Folds a chunk stream into totals and the root digest: SHA-256 over the concatenated raw
 * chunk-digest bytes, matching the Python layer. Retention is bounded by 32 bytes per chunk -
 * for a 5M-line tree at 1024-record chunks that is about 156 KiB of digest state, not the
 * records themselves.
 */
export async function summarize(
  root: string,
  chunks: AsyncIterable<ManifestChunk>,
): Promise<ManifestSummary> {
  const parts: Uint8Array[] = [];
  let totalFiles = 0;
  let totalBytes = 0;
  let totalLines = 0;
  let chunkCount = 0;
  for await (const chunk of chunks) {
    parts.push(hexToBytes(chunk.digest, chunk.index));
    totalFiles += chunk.records.length;
    totalBytes += chunk.bytes;
    totalLines += chunk.lines;
    chunkCount += 1;
  }
  const all = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const part of parts) {
    all.set(part, offset);
    offset += part.length;
  }
  return {
    root,
    total_files: totalFiles,
    total_bytes: totalBytes,
    total_lines: totalLines,
    chunk_count: chunkCount,
    root_digest: await sha256Hex(all),
  };
}

function hexToBytes(hex: string, chunkIndex: number): Uint8Array {
  if (!/^[0-9a-f]*$/.test(hex) || hex.length % 2 !== 0) {
    throw new ManifestError(`chunk ${chunkIndex} digest is not valid lowercase hex`);
  }
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

/**
 * The bound-model peak number of records held at once: never the whole stream, always at most
 * one chunk. Tests assert this instead of trusting generator laziness.
 */
export function peakRecordState(fileCount: number, chunkRecords: number): number {
  return Math.min(fileCount, chunkRecords);
}
