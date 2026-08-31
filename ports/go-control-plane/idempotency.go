package controlplane

// IdempotencyWindow recognizes retried submissions while a task is unsettled.
//
// The window closes with settlement: bounded memory beats an unbounded key
// map, and the closure is counted so it is never silent — after CloseTask the
// same key would be admitted as a new task, and that state change is visible
// in Evictions rather than hidden.
type IdempotencyWindow struct {
	byKey     map[IdempotencyKey]TaskID
	byTask    map[TaskID]IdempotencyKey
	evictions uint64
}

// NewIdempotencyWindow constructs an empty window.
func NewIdempotencyWindow() *IdempotencyWindow {
	return &IdempotencyWindow{
		byKey:  make(map[IdempotencyKey]TaskID),
		byTask: make(map[TaskID]IdempotencyKey),
	}
}

// Register records an admission or reports the earlier one. A duplicate
// returns the original task and changes nothing: the caller must treat it as
// the same submission, not as permission to enqueue twice.
func (w *IdempotencyWindow) Register(key IdempotencyKey, task TaskID) (existing TaskID, duplicate bool) {
	if prior, ok := w.byKey[key]; ok {
		return prior, true
	}
	w.byKey[key] = task
	w.byTask[task] = key
	return TaskID{}, false
}

// CloseTask ends the window for a settled task and reports whether an entry
// was actually closed. Closing an unknown task is reported, not counted, so
// the eviction counter stays an exact measure of windows that shut.
func (w *IdempotencyWindow) CloseTask(task TaskID) bool {
	key, ok := w.byTask[task]
	if !ok {
		return false
	}
	delete(w.byTask, task)
	delete(w.byKey, key)
	w.evictions++
	return true
}

// Evictions counts closed windows. Every dedupe drop is observable here
// instead of vanishing into an unbounded map.
func (w *IdempotencyWindow) Evictions() uint64 { return w.evictions }

// Live counts open windows.
func (w *IdempotencyWindow) Live() int { return len(w.byKey) }
