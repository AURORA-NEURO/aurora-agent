package controlplane

import "testing"

func TestDuplicateKeyWithinWindowReturnsOriginalTaskAndChangesNothing(t *testing.T) {
	w := NewIdempotencyWindow()
	task1, _ := NewTaskID(7)
	task2, _ := NewTaskID(8)
	key := DeriveIdempotencyKey(100, 200)

	existing, dup := w.Register(key, task1)
	if dup || existing != (TaskID{}) {
		t.Fatalf("first registration reported duplicate %d", existing.Raw())
	}
	existing, dup = w.Register(key, task2)
	if !dup || existing != task1 {
		t.Fatalf("replayed key admitted as new or named the wrong task: %v %v", dup, existing)
	}
	if w.Live() != 1 {
		t.Fatalf("duplicate registration grew the window: live=%d", w.Live())
	}
}

func TestSettlementClosesTheWindowAndCountsTheEviction(t *testing.T) {
	w := NewIdempotencyWindow()
	task, _ := NewTaskID(9)
	key := DeriveIdempotencyKey(1, 2)
	w.Register(key, task)

	if !w.CloseTask(task) {
		t.Fatal("settled task's window did not close")
	}
	if w.Evictions() != 1 || w.Live() != 0 {
		t.Fatalf("closure not counted: evictions=%d live=%d", w.Evictions(), w.Live())
	}
	fresh, _ := NewTaskID(10)
	if _, dup := w.Register(key, fresh); dup {
		t.Fatal("closed window still recognizes the key; a retried submission after settlement would be silently swallowed")
	}
}

func TestCloseOfUnknownTaskIsReportedButNeverCounted(t *testing.T) {
	w := NewIdempotencyWindow()
	task, _ := NewTaskID(11)
	stranger, _ := NewTaskID(12)
	w.Register(DeriveIdempotencyKey(3, 4), task)

	if w.CloseTask(stranger) {
		t.Fatal("closing an unregistered task reported success")
	}
	if w.Evictions() != 0 {
		t.Fatalf("unknown close inflated evictions to %d", w.Evictions())
	}
	if !w.CloseTask(task) || w.CloseTask(task) {
		t.Fatal("double close of one window succeeded twice")
	}
	if w.Evictions() != 1 {
		t.Fatalf("evictions %d after double close, want exactly 1", w.Evictions())
	}
}
