/**
 * Bounded FIFO queue with explicit backpressure, transcribed from
 * integrations/scale-5m/aurora_scale/fleet.py (BoundedQueue).
 *
 * Push never drops, evicts, grows, or awaits: a full queue returns a typed backpressure
 * result and the caller decides. The queue is async-agnostic in the strict sense - items may
 * be promises, but admission control is synchronous and the queue never schedules, timers or
 * microtasks of its own. Awaiting drained items belongs to the scheduler above it.
 */

export type QueueCapacityError = RangeError & { readonly name: "QueueCapacityError" };

export type PushResult =
  | { readonly ok: true }
  | { readonly ok: false; readonly reason: "backpressure"; readonly capacity: number };

export type PopResult<T> =
  | { readonly ok: true; readonly item: T }
  | { readonly ok: false; readonly reason: "empty" };

function queueCapacityError(capacity: number): QueueCapacityError {
  const error = new RangeError(
    `queue capacity must be a positive safe integer, got ${capacity}`,
  ) as QueueCapacityError;
  return error;
}

export class BoundedQueue<T> {
  private readonly ring: Array<T | undefined>;
  private head = 0;
  private count = 0;
  private highWaterMark = 0;

  constructor(readonly capacity: number) {
    if (!Number.isSafeInteger(capacity) || capacity <= 0) throw queueCapacityError(capacity);
    this.ring = new Array<T | undefined>(capacity);
  }

  get length(): number {
    return this.count;
  }

  /**
   * The highest length ever reached: monotone, and the observable that the bound held under
   * load - tests assert this rather than trusting the capacity parameter.
   */
  get highWater(): number {
    return this.highWaterMark;
  }

  /** Enqueues or reports backpressure. Never blocks, never drops, never grows. */
  push(item: T): PushResult {
    if (this.count >= this.capacity) {
      return { ok: false, reason: "backpressure", capacity: this.capacity };
    }
    const slot = (this.head + this.count) % this.capacity;
    this.ring[slot] = item;
    this.count += 1;
    if (this.count > this.highWaterMark) this.highWaterMark = this.count;
    return { ok: true };
  }

  /**
   * Dequeues the oldest item, or reports emptiness as data. The slot is cleared so queued
   * values do not outlive their dequeue.
   */
  pop(): PopResult<T> {
    if (this.count === 0) return { ok: false, reason: "empty" };
    const item = this.ring[this.head] as T;
    this.ring[this.head] = undefined;
    this.head = (this.head + 1) % this.capacity;
    this.count -= 1;
    return { ok: true, item };
  }
}
