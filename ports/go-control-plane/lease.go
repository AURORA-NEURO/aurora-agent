package controlplane

import (
	"errors"
	"fmt"
	"sort"
)

// ErrZeroTTL reports a lease grant or renewal whose TTL is zero: such a lease
// would expire before use, so it is refused instead of silently created.
var ErrZeroTTL = errors.New("lease ttl must be positive")

// ErrUnknownTask reports that no lease is recorded for the task at all.
var ErrUnknownTask = errors.New("no lease recorded for this task")

// HeldByOtherError reports a refused grant and names the current holder, so
// the caller learns who owns the task rather than merely that it failed.
type HeldByOtherError struct {
	Task   TaskID
	Holder AgentID
}

func (e *HeldByOtherError) Error() string {
	return fmt.Sprintf("task %s already leased to %s", e.Task, e.Holder)
}

// EpochMismatchError reports that the presented epoch is not current — the
// lease expired and was re-granted, so a stale generation can neither renew
// nor release what it once held.
type EpochMismatchError struct {
	Task      TaskID
	Presented LeaseEpoch
	Current   LeaseEpoch
}

func (e *EpochMismatchError) Error() string {
	return fmt.Sprintf("lease epoch %s no longer current for %s (current %s)",
		e.Presented, e.Task, e.Current)
}

// LeaseHandle carries the identity of one live lease. Go has no move
// semantics, so unlike the Rust handle this value is copyable; exclusivity is
// therefore enforced by epoch checks on every mutating call rather than by
// consuming the token. The consequence is stated in the docs: a caller that
// retains a handle after release gets an EpochMismatchError, not a compile
// error.
type LeaseHandle struct {
	task  TaskID
	agent AgentID
	epoch LeaseEpoch
}

// Task returns the leased task.
func (h LeaseHandle) Task() TaskID { return h.task }

// Agent returns the holder the lease was granted to.
func (h LeaseHandle) Agent() AgentID { return h.agent }

// Epoch returns the generation minted for this lease.
func (h LeaseHandle) Epoch() LeaseEpoch { return h.epoch }

type activeLease struct {
	holder    AgentID
	epoch     LeaseEpoch
	expiresAt uint64
}

// ExpiredLease reports one lapsed lease for retry handling.
type ExpiredLease struct {
	Task        TaskID
	Agent       AgentID
	EndedAtTick uint64
}

// LeaseTable grants exclusive task leases with TTL expiry. At most one live
// lease per task exists because Grant refuses while one is held, and epochs
// advance monotonically across the whole table, so a stale handle from an
// expired generation can never be mistaken for the current one.
//
// Expiry is how the control plane detects crashed or silently-dead workers:
// the lease simply outlives its TTL and the task returns to the retry path.
type LeaseTable struct {
	byTask    map[TaskID]activeLease
	nextEpoch uint64
}

// NewLeaseTable constructs an empty table.
func NewLeaseTable() *LeaseTable {
	return &LeaseTable{byTask: make(map[TaskID]activeLease)}
}

// Grant grants an exclusive lease or fails naming the current holder.
func (t *LeaseTable) Grant(task TaskID, agent AgentID, now, ttlTicks uint64) (LeaseHandle, error) {
	if ttlTicks == 0 {
		return LeaseHandle{}, ErrZeroTTL
	}
	if active, held := t.byTask[task]; held {
		return LeaseHandle{}, &HeldByOtherError{Task: task, Holder: active.holder}
	}
	t.nextEpoch++
	epoch := newLeaseEpoch(t.nextEpoch)
	t.byTask[task] = activeLease{holder: agent, epoch: epoch, expiresAt: now + ttlTicks}
	return LeaseHandle{task: task, agent: agent, epoch: epoch}, nil
}

// Renew extends a live lease. The fresh handle replaces the presented one;
// because Go copies values, both remain usable in the caller's scope, but only
// the epoch the table currently records can ever renew or release again.
func (t *LeaseTable) Renew(h LeaseHandle, now, ttlTicks uint64) (LeaseHandle, error) {
	if ttlTicks == 0 {
		return LeaseHandle{}, ErrZeroTTL
	}
	active, ok := t.byTask[h.task]
	if !ok {
		return LeaseHandle{}, ErrUnknownTask
	}
	if active.epoch != h.epoch {
		return LeaseHandle{}, &EpochMismatchError{Task: h.task, Presented: h.epoch, Current: active.epoch}
	}
	active.expiresAt = now + ttlTicks
	t.byTask[h.task] = active
	return h, nil
}

// Release ends the lease identified by the handle's task and epoch.
func (t *LeaseTable) Release(h LeaseHandle) error {
	return t.ReleaseBy(h.task, h.epoch)
}

// ReleaseBy is the epoch-keyed release for callers that tracked the epoch
// without retaining a handle (the settlement path). A stale epoch is rejected,
// so a late completion cannot free a lease it no longer owns.
func (t *LeaseTable) ReleaseBy(task TaskID, epoch LeaseEpoch) error {
	active, ok := t.byTask[task]
	if !ok {
		return ErrUnknownTask
	}
	if active.epoch != epoch {
		return &EpochMismatchError{Task: task, Presented: epoch, Current: active.epoch}
	}
	delete(t.byTask, task)
	return nil
}

// ExpireBefore drops every lease whose TTL has passed by now and reports them
// in ascending task order, matching the ordered maps of the Rust reference so
// two runs observe identical expiry streams.
func (t *LeaseTable) ExpireBefore(now uint64) []ExpiredLease {
	var due []ExpiredLease
	for task, active := range t.byTask {
		if active.expiresAt <= now {
			due = append(due, ExpiredLease{Task: task, Agent: active.holder, EndedAtTick: active.expiresAt})
		}
	}
	sort.Slice(due, func(i, j int) bool { return due[i].Task.raw < due[j].Task.raw })
	for _, d := range due {
		delete(t.byTask, d.Task)
	}
	return due
}

// Holder reports the current holder of a live lease, if any.
func (t *LeaseTable) Holder(task TaskID) (AgentID, bool) {
	active, ok := t.byTask[task]
	return active.holder, ok
}

// Live counts live leases.
func (t *LeaseTable) Live() int { return len(t.byTask) }

// ExpiryOf reports when the task's lease lapses.
func (t *LeaseTable) ExpiryOf(task TaskID) (uint64, bool) {
	active, ok := t.byTask[task]
	return active.expiresAt, ok
}
