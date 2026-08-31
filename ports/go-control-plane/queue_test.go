package controlplane

import (
	"errors"
	"testing"
)

func TestFullQueueReportsBackpressureInsteadOfDropping(t *testing.T) {
	q, err := NewBoundedQueue[int](2)
	if err != nil {
		t.Fatal(err)
	}
	if err := q.Push(1); err != nil {
		t.Fatal(err)
	}
	if err := q.Push(2); err != nil {
		t.Fatal(err)
	}
	var full *BackpressureError
	err = q.Push(3)
	if !errors.As(err, &full) || full.Capacity != 2 {
		t.Fatalf("third push: got %v, want capacity-2 backpressure", err)
	}
	if v, _ := q.Pop(); v != 1 {
		t.Fatalf("pop returned %d, want FIFO head 1", v)
	}
	if err := q.Push(3); err != nil {
		t.Fatalf("push after pop: %v", err)
	}
}

func TestFIFOOrderIsPreservedExactly(t *testing.T) {
	q, _ := NewBoundedQueue[int](4)
	for i := 0; i < 4; i++ {
		if err := q.Push(i); err != nil {
			t.Fatal(err)
		}
	}
	for want := 0; want < 4; want++ {
		got, ok := q.Pop()
		if !ok || got != want {
			t.Fatalf("position %d: got %d (ok=%v)", want, got, ok)
		}
	}
}

func TestHighWaterRecordsPeakButNeverCapacityWhenUnused(t *testing.T) {
	q, _ := NewBoundedQueue[int](10)
	_ = q.Push(1)
	_ = q.Push(2)
	_, _ = q.Pop()
	if q.HighWater() != 2 {
		t.Fatalf("high water %d, want peak 2", q.HighWater())
	}
	if q.Len() != 1 {
		t.Fatalf("len %d after one pop", q.Len())
	}
}

func TestNonPositiveCapacityRefusesToConstruct(t *testing.T) {
	if _, err := NewBoundedQueue[int](0); err == nil {
		t.Fatal("zero-capacity queue constructed")
	}
	if _, err := NewBoundedQueue[int](-3); err == nil {
		t.Fatal("negative-capacity queue constructed")
	}
}

func TestPopFromEmptyQueueReportsAbsenceRatherThanBlockingOrPanic(t *testing.T) {
	q, _ := NewBoundedQueue[string](1)
	if _, ok := q.Pop(); ok {
		t.Fatal("empty pop produced a value")
	}
	if err := q.Push("x"); err != nil {
		t.Fatal(err)
	}
	v, ok := q.Pop()
	if !ok || v != "x" {
		t.Fatalf("pop lost the item: %q %v", v, ok)
	}
	if _, ok := q.Pop(); ok {
		t.Fatal("drained queue produced a second value")
	}
}

func TestBoundHoldsUnderLoadWithHighWaterAsTheWitness(t *testing.T) {
	q, _ := NewBoundedQueue[uint64](16)
	rejected := 0
	for i := uint64(0); i < 1000; i++ {
		if err := q.Push(i); err != nil {
			var full *BackpressureError
			if errors.As(err, &full) && full.Capacity == 16 {
				rejected++
			} else {
				t.Fatalf("push %d failed oddly: %v", i, err)
			}
		}
	}
	if q.HighWater() != 16 || q.Len() != 16 {
		t.Fatalf("bound broken: high water %d len %d", q.HighWater(), q.Len())
	}
	if rejected != 984 {
		t.Fatalf("%d rejections, want exactly 984 for a 1000-push run against cap 16", rejected)
	}
}
