package controlplane

import (
	"bytes"
	"testing"
)

func TestSnapshotJSONKeyOrderMatchesTheCPythonScaleLayerExactly(t *testing.T) {
	loadParity(t)
	var keys []string
	paritySection(t, "telemetry_snapshot_keys", &keys)
	tel := &Telemetry{}
	tel.Dispatch()
	tel.Dispatch()
	tel.Complete()
	tel.RejectBackpressure()
	tel.LeaseExpired()

	line, err := tel.SnapshotJSON()
	if err != nil {
		t.Fatal(err)
	}
	// The vector lists the Python dict's keys sorted; our encoder must emit
	// them in that same order.
	cursor := 0
	for _, key := range keys {
		token := `"` + key + `":`
		at := bytes.Index(line[cursor:], []byte(token))
		if at < 0 {
			t.Fatalf("snapshot %s missing %s after offset %d", line, token, cursor)
		}
		cursor += at + len(token)
	}
}

func TestCompletionCannotDriveInFlightNegativeOrPeakDrift(t *testing.T) {
	tel := &Telemetry{}
	tel.Complete()
	tel.Complete()
	snap := tel.Snapshot()
	if snap.InFlight != 0 || snap.Completed != 2 {
		t.Fatalf("over-completion corrupted counters: %+v", snap)
	}
	tel.Dispatch()
	tel.Dispatch()
	tel.Complete()
	snap = tel.Snapshot()
	if snap.InFlight != 1 || snap.PeakInFlight != 2 {
		t.Fatalf("peak/in-flight wrong: %+v", snap)
	}
}

func TestBackpressureAndExpiryCountersAreObservableSeparately(t *testing.T) {
	tel := &Telemetry{}
	tel.RejectBackpressure()
	tel.LeaseExpired()
	line, _ := tel.SnapshotJSON()
	want := `{"completed":0,"dispatched":0,"in_flight":0,"lease_expiries":1,` +
		`"peak_in_flight":0,"rejected_backpressure":1,"submitted":0}`
	if string(line) != want {
		t.Fatalf("snapshot bytes drifted:\n got %s\nwant %s", line, want)
	}
}
