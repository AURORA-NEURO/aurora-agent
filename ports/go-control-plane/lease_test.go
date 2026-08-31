package controlplane

import (
	"errors"
	"testing"
)

func idsFor(t *testing.T, task, agent uint64) (TaskID, AgentID) {
	t.Helper()
	tid, err := NewTaskID(task)
	if err != nil {
		t.Fatal(err)
	}
	aid, err := NewAgentID(agent)
	if err != nil {
		t.Fatal(err)
	}
	return tid, aid
}

func TestSecondGrantWhileHeldNamesTheHolderRatherThanSucceeding(t *testing.T) {
	table := NewLeaseTable()
	task, x := idsFor(t, 1, 1)
	_, y := idsFor(t, 2, 2)
	h, err := table.Grant(task, x, 0, 10)
	if err != nil {
		t.Fatalf("first grant: %v", err)
	}
	var held *HeldByOtherError
	_, err = table.Grant(task, y, 1, 10)
	if !errors.As(err, &held) || held.Holder != x {
		t.Fatalf("second grant: got %v, want holder named", err)
	}
	if err := table.Release(h); err != nil {
		t.Fatalf("release: %v", err)
	}
	if _, err := table.Grant(task, y, 2, 10); err != nil {
		t.Fatalf("task did not free after release: %v", err)
	}
}

func TestAStaleGenerationCannotRenewOrReleaseAfterRegrant(t *testing.T) {
	table := NewLeaseTable()
	task, agent := idsFor(t, 9, 9)
	h1, _ := table.Grant(task, agent, 0, 1)
	oldEpoch := h1.Epoch()
	if expired := table.ExpireBefore(5); len(expired) != 1 {
		t.Fatalf("expected one expiry, got %d", len(expired))
	}
	if _, err := table.Renew(h1, 6, 10); !errors.Is(err, ErrUnknownTask) {
		t.Fatalf("renew of expired generation: got %v", err)
	}
	h2, err := table.Grant(task, agent, 6, 10)
	if err != nil {
		t.Fatalf("re-grant: %v", err)
	}
	if h2.Epoch() == oldEpoch {
		t.Fatal("re-grant reused the expired generation's epoch")
	}
	var mismatch *EpochMismatchError
	err = table.ReleaseBy(task, oldEpoch)
	if !errors.As(err, &mismatch) {
		t.Fatalf("stale release: got %v, want epoch mismatch", err)
	}
	if mismatch.Current != h2.Epoch() {
		t.Fatalf("mismatch reported wrong current epoch: %s", mismatch.Current)
	}
}

func TestExpiryReportsOnlyWhatActuallyLapsedWithItsEndTick(t *testing.T) {
	table := NewLeaseTable()
	t1, a1 := idsFor(t, 11, 11)
	t2, a2 := idsFor(t, 12, 12)
	if _, err := table.Grant(t1, a1, 0, 10); err != nil {
		t.Fatal(err)
	}
	h2, err := table.Grant(t2, a2, 0, 100)
	if err != nil {
		t.Fatal(err)
	}
	expired := table.ExpireBefore(10)
	if len(expired) != 1 {
		t.Fatalf("expired %d leases at tick 10, want exactly 1", len(expired))
	}
	e := expired[0]
	if e.Task != t1 || e.Agent != a1 || e.EndedAtTick != 10 {
		t.Fatalf("wrong lapse reported: %+v", e)
	}
	if table.Live() != 1 {
		t.Fatalf("survivor lost: live=%d", table.Live())
	}
	if err := table.Release(h2); err != nil {
		t.Fatalf("release survivor: %v", err)
	}
}

func TestZeroTTLIsRejectedBeforeAnyStateChanges(t *testing.T) {
	table := NewLeaseTable()
	task, agent := idsFor(t, 21, 21)
	if _, err := table.Grant(task, agent, 0, 0); !errors.Is(err, ErrZeroTTL) {
		t.Fatalf("zero-ttl grant: got %v", err)
	}
	if table.Live() != 0 {
		t.Fatal("rejected grant left state behind")
	}
	h, _ := table.Grant(task, agent, 0, 5)
	if _, err := table.Renew(h, 1, 0); !errors.Is(err, ErrZeroTTL) {
		t.Fatalf("zero-ttl renew: got %v", err)
	}
	if expiry, _ := table.ExpiryOf(task); expiry != 5 {
		t.Fatalf("failed renewal mutated expiry: %d", expiry)
	}
}

func TestEpochsAdvanceMonotonicallyAcrossTasksAndGenerations(t *testing.T) {
	table := NewLeaseTable()
	t1, a1 := idsFor(t, 31, 31)
	t2, a2 := idsFor(t, 32, 32)
	h1a, _ := table.Grant(t1, a1, 0, 5)
	h2, _ := table.Grant(t2, a2, 0, 5)
	if h2.Epoch().Raw() <= h1a.Epoch().Raw() {
		t.Fatalf("epoch did not advance across tasks: %s then %s", h1a.Epoch(), h2.Epoch())
	}
	table.Release(h1a)
	h1b, _ := table.Grant(t1, a1, 0, 5)
	if h1b.Epoch().Raw() <= h2.Epoch().Raw() {
		t.Fatalf("re-grant did not advance past the other live lease: %s vs %s",
			h1b.Epoch(), h2.Epoch())
	}
}

func TestHolderAndExpiryObservationsMatchRealityWithoutMutating(t *testing.T) {
	table := NewLeaseTable()
	task, agent := idsFor(t, 41, 41)
	if _, ok := table.Holder(task); ok {
		t.Fatal("ungranted task reports a holder")
	}
	h, _ := table.Grant(task, agent, 3, 7)
	got, ok := table.Holder(task)
	if !ok || got != agent {
		t.Fatalf("holder observation wrong: %s %v", got, ok)
	}
	if expiry, _ := table.ExpiryOf(task); expiry != 10 {
		t.Fatalf("expiry observation wrong: %d", expiry)
	}
	if err := table.Release(h); err != nil {
		t.Fatal(err)
	}
	if _, ok := table.ExpiryOf(task); ok {
		t.Fatal("released task still reports an expiry")
	}
}
