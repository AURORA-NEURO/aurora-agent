package controlplane

import (
	"encoding/json"
)

// Telemetry accumulates bounded-fleet counters. Every counter is observable;
// nothing about queue rejections, lease expiries or in-flight peaks is folded
// into a generic "errors" bucket that would make a refusal look like work.
type Telemetry struct {
	submitted            uint64
	dispatched           uint64
	completed            uint64
	rejectedBackpressure uint64
	leaseExpiries        uint64
	inFlight             uint64
	peakInFlight         uint64
}

// TelemetrySnapshot is the exported shape of the counters. Fields are declared
// in alphabetical order on purpose: encoding/json emits struct fields in
// declaration order, so the JSON snapshot is byte-stable and its key order
// matches the Python scale layer's sort_keys dumps exactly.
type TelemetrySnapshot struct {
	Completed            uint64 `json:"completed"`
	Dispatched           uint64 `json:"dispatched"`
	InFlight             uint64 `json:"in_flight"`
	LeaseExpiries        uint64 `json:"lease_expiries"`
	PeakInFlight         uint64 `json:"peak_in_flight"`
	RejectedBackpressure uint64 `json:"rejected_backpressure"`
	Submitted            uint64 `json:"submitted"`
}

// Dispatch records one admitted dispatch.
func (t *Telemetry) Dispatch() {
	t.submitted++
	t.dispatched++
	t.inFlight++
	if t.inFlight > t.peakInFlight {
		t.peakInFlight = t.inFlight
	}
}

// Complete records one settled attempt. The clamp mirrors the reference: an
// over-counted completion cannot drive in-flight negative and disguise itself
// as consistency.
func (t *Telemetry) Complete() {
	t.completed++
	if t.inFlight > 0 {
		t.inFlight--
	}
}

// RejectBackpressure records one admission refused at a full queue.
func (t *Telemetry) RejectBackpressure() { t.rejectedBackpressure++ }

// LeaseExpired records one lease that lapsed to TTL.
func (t *Telemetry) LeaseExpired() { t.leaseExpiries++ }

// Snapshot returns the counters as a value.
func (t *Telemetry) Snapshot() TelemetrySnapshot {
	return TelemetrySnapshot{
		Completed:            t.completed,
		Dispatched:           t.dispatched,
		InFlight:             t.inFlight,
		LeaseExpiries:        t.leaseExpiries,
		PeakInFlight:         t.peakInFlight,
		RejectedBackpressure: t.rejectedBackpressure,
		Submitted:            t.submitted,
	}
}

// SnapshotJSON renders the snapshot with stable key order. Maps are avoided
// deliberately: Go randomizes map iteration, which would make byte-for-byte
// telemetry comparisons impossible.
func (t *Telemetry) SnapshotJSON() ([]byte, error) {
	return json.Marshal(t.Snapshot())
}
