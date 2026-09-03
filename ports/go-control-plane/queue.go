package controlplane

import (
	"fmt"
)

// BackpressureError is the only failure a bounded push can produce, carrying
// the capacity needed to report it.
type BackpressureError struct {
	Capacity int
}

func (e *BackpressureError) Error() string {
	return fmt.Sprintf("queue at capacity %d", e.Capacity)
}

// BoundedQueue is a FIFO with a hard capacity. Push returns *BackpressureError
// when full and the caller decides — the queue never drops, never evicts, and
// never grows. The high-water mark is tracked so tests can assert the bound
// held under load rather than trusting the capacity parameter.
type BoundedQueue[T any] struct {
	items     []T
	head      int
	count     int
	capacity  int
	highWater int
}

// NewBoundedQueue constructs a queue. A non-positive capacity is a
// construction error here rather than the reference's panic, because Go
// convention routes programmer errors that are data-dependent through error
// returns; the invariant enforced is identical.
func NewBoundedQueue[T any](capacity int) (*BoundedQueue[T], error) {
	if capacity <= 0 {
		return nil, fmt.Errorf("queue capacity must be positive, got %d", capacity)
	}
	return &BoundedQueue[T]{items: make([]T, capacity), capacity: capacity}, nil
}

// Capacity is the configured bound.
func (q *BoundedQueue[T]) Capacity() int { return q.capacity }

// Len is the current occupancy.
func (q *BoundedQueue[T]) Len() int { return q.count }

// HighWater is the highest length ever reached: monotone, and the observable
// for "the bound was never exceeded".
func (q *BoundedQueue[T]) HighWater() int { return q.highWater }

// Push enqueues or reports backpressure. Never blocks, never drops.
func (q *BoundedQueue[T]) Push(item T) error {
	if q.count >= q.capacity {
		return &BackpressureError{Capacity: q.capacity}
	}
	q.items[(q.head+q.count)%q.capacity] = item
	q.count++
	if q.count > q.highWater {
		q.highWater = q.count
	}
	return nil
}

// Pop dequeues the oldest item; ok is false when empty. The slot is zeroed so
// queued pointer values do not outlive their dequeue.
func (q *BoundedQueue[T]) Pop() (item T, ok bool) {
	if q.count == 0 {
		return item, false
	}
	item = q.items[q.head]
	var zero T
	q.items[q.head] = zero
	q.head = (q.head + 1) % q.capacity
	q.count--
	return item, true
}
