/** Shared runtime-local pacing for reviewed NCBI E-utilities adapters. */

const NativePromise = globalThis.Promise;
const nativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const nativeMonotonicNow = globalThis.performance && typeof globalThis.performance.now === "function"
  ? globalThis.performance.now.bind(globalThis.performance)
  : Date.now.bind(Date);
const MIN_REQUEST_INTERVAL_MS = 340;

let requestQueue: Promise<void> = NativePromise.resolve();
let lastDispatchAt: number | null = null;

export async function acquireNcbiRequestSlot(): Promise<void> {
  const predecessor = requestQueue;
  let release!: () => void;
  requestQueue = new NativePromise<void>((resolve) => { release = resolve; });
  await predecessor;
  try {
    if (lastDispatchAt !== null) {
      while (true) {
        const remaining = MIN_REQUEST_INTERVAL_MS - (nativeMonotonicNow() - lastDispatchAt);
        if (remaining <= 0) break;
        await new NativePromise<void>((resolve) => nativeSetTimeout(resolve, remaining));
      }
    }
    lastDispatchAt = nativeMonotonicNow();
  } finally {
    release();
  }
}
