/**
 * Bounded-fleet telemetry counters, transcribed from
 * integrations/scale-5m/aurora_scale/fleet.py (Telemetry).
 *
 * Every counter is observable; nothing about queue rejections or in-flight peaks is folded
 * into a generic "errors" bucket that would make a refusal look like work. The snapshot is
 * serialized through the canonical JSON serializer, so its key order is alphabetical and its
 * bytes are identical to the Python layer's sort_keys dumps - the property parity vectors pin.
 */

import { canonicalJsonString, type CanonicalValue } from "./canonical.js";

export interface TelemetrySnapshot {
  readonly completed: number;
  readonly dispatched: number;
  readonly in_flight: number;
  readonly lease_expiries: number;
  readonly peak_in_flight: number;
  readonly rejected_backpressure: number;
  readonly submitted: number;
}

export class Telemetry {
  private submittedCount = 0;
  private dispatchedCount = 0;
  private completedCount = 0;
  private rejectedBackpressureCount = 0;
  private leaseExpiriesCount = 0;
  private inFlight = 0;
  private peakInFlight = 0;

  /** Records one admitted dispatch. */
  dispatch(): void {
    this.submittedCount += 1;
    this.dispatchedCount += 1;
    this.inFlight += 1;
    if (this.inFlight > this.peakInFlight) this.peakInFlight = this.inFlight;
  }

  /**
   * Records one settled attempt. The clamp mirrors the reference: an over-counted completion
   * cannot drive in-flight negative and disguise itself as consistency.
   */
  complete(): void {
    this.completedCount += 1;
    if (this.inFlight > 0) this.inFlight -= 1;
  }

  /** Records one admission refused at a full queue. */
  rejectBackpressure(): void {
    this.rejectedBackpressureCount += 1;
  }

  /** Records one lease that lapsed to TTL. */
  leaseExpired(): void {
    this.leaseExpiriesCount += 1;
  }

  snapshot(): TelemetrySnapshot {
    return {
      completed: this.completedCount,
      dispatched: this.dispatchedCount,
      in_flight: this.inFlight,
      lease_expiries: this.leaseExpiriesCount,
      peak_in_flight: this.peakInFlight,
      rejected_backpressure: this.rejectedBackpressureCount,
      submitted: this.submittedCount,
    };
  }

  /**
   * Canonical bytes of the snapshot. The serializer sorts keys by code point, so byte-stability
   * does not depend on object literal order or engine map quirks.
   */
  snapshotJSON(): string {
    return canonicalJsonString(this.snapshot() as unknown as CanonicalValue);
  }
}
