package controlplane

import (
	"errors"
	"fmt"
	"sort"
)

// AdapterState mirrors the Python scale layer's descriptor states exactly.
// supported and partial are reserved for the small local Aurora MCP surfaces;
// descriptor-only means a future contract can be registered without pretending
// connectivity; refused means intentionally not implemented.
type AdapterState string

const (
	StateSupported      AdapterState = "supported"
	StatePartial        AdapterState = "partial"
	StateDescriptorOnly AdapterState = "descriptor-only"
	StateRefused        AdapterState = "refused"
)

// Unsupported is true for the two states that must never be presented as live
// support: a refusal and a shape-only description look nothing like a working
// connector, and this predicate keeps that distinction in one place.
func (s AdapterState) Unsupported() bool {
	return s == StateDescriptorOnly || s == StateRefused
}

// AdapterDescriptor describes one (platform, protocol) pair. It records what a
// platform speaks; it never claims a live connection to it.
type AdapterDescriptor struct {
	Platform     string       `json:"platform"`
	Protocol     string       `json:"protocol"`
	State        AdapterState `json:"state"`
	Capabilities []string     `json:"capabilities"`
	Notes        string       `json:"notes"`
}

// DescriptorError reports an invalid registration with the reason spelled out.
type DescriptorError struct {
	Platform, Protocol, Reason string
}

func (e *DescriptorError) Error() string {
	return fmt.Sprintf("invalid descriptor %s/%s: %s", e.Platform, e.Protocol, e.Reason)
}

// UnknownDescriptorError reports a lookup miss. A missing adapter is an error,
// never an empty struct that could be mistaken for a valid descriptor.
type UnknownDescriptorError struct {
	Platform, Protocol string
}

func (e *UnknownDescriptorError) Error() string {
	return fmt.Sprintf("no descriptor for %s/%s", e.Platform, e.Protocol)
}

// UnsupportedAdapterError reports a RequireLive refusal carrying the state and
// notes, so the caller sees why the surface cannot be used rather than a bare
// failure.
type UnsupportedAdapterError struct {
	Platform, Protocol string
	State              AdapterState
	Notes              string
}

func (e *UnsupportedAdapterError) Error() string {
	return fmt.Sprintf("%s/%s is %s: %s", e.Platform, e.Protocol, e.State, e.Notes)
}

type descriptorKey struct{ platform, protocol string }

// AdapterRegistry stores compact descriptors keyed by (platform, protocol).
// Lookup is exact: there is no prefix or wildcard matching, because a wildcard
// hit that resolves to a platform that only resembles the request is worse
// than an honest miss.
type AdapterRegistry struct {
	items map[descriptorKey]AdapterDescriptor
}

// NewAdapterRegistry constructs an empty registry.
func NewAdapterRegistry() *AdapterRegistry {
	return &AdapterRegistry{items: make(map[descriptorKey]AdapterDescriptor)}
}

// Register validates and stores one descriptor. Capabilities must already be
// sorted and unique — the caller's declaration order is part of the recorded
// contract, so silent reordering here would hide disagreements between
// registrars.
func (r *AdapterRegistry) Register(d AdapterDescriptor) error {
	if d.Platform == "" || d.Protocol == "" {
		return &DescriptorError{Platform: d.Platform, Protocol: d.Protocol, Reason: "platform and protocol are required"}
	}
	if !sortedUnique(d.Capabilities) {
		return &DescriptorError{
			Platform: d.Platform, Protocol: d.Protocol,
			Reason: "capabilities must be sorted and unique",
		}
	}
	key := descriptorKey{d.Platform, d.Protocol}
	if _, exists := r.items[key]; exists {
		return &DescriptorError{Platform: d.Platform, Protocol: d.Protocol, Reason: "duplicate adapter descriptor"}
	}
	r.items[key] = d
	return nil
}

// Get returns the exact descriptor for the pair or a typed miss.
func (r *AdapterRegistry) Get(platform, protocol string) (AdapterDescriptor, error) {
	d, ok := r.items[descriptorKey{platform, protocol}]
	if !ok {
		return AdapterDescriptor{}, &UnknownDescriptorError{Platform: platform, Protocol: protocol}
	}
	return d, nil
}

// RequireLive returns the descriptor only when its state can honestly be
// presented as usable support; descriptor-only and refused states are typed
// refusals carrying their notes.
func (r *AdapterRegistry) RequireLive(platform, protocol string) (AdapterDescriptor, error) {
	d, err := r.Get(platform, protocol)
	if err != nil {
		return AdapterDescriptor{}, err
	}
	if d.State.Unsupported() {
		return AdapterDescriptor{}, &UnsupportedAdapterError{
			Platform: d.Platform, Protocol: d.Protocol, State: d.State, Notes: d.Notes,
		}
	}
	return d, nil
}

// Snapshot returns every descriptor ordered by (platform, protocol), so
// callers observe the registry deterministically regardless of insertion
// order — Go map iteration would otherwise leak randomness into outputs.
func (r *AdapterRegistry) Snapshot() []AdapterDescriptor {
	out := make([]AdapterDescriptor, 0, len(r.items))
	for _, d := range r.items {
		out = append(out, d)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Platform != out[j].Platform {
			return out[i].Platform < out[j].Platform
		}
		return out[i].Protocol < out[j].Protocol
	})
	return out
}

// Len counts registered descriptors.
func (r *AdapterRegistry) Len() int { return len(r.items) }

// DefaultRegistry mirrors the Python scale layer's default inventory: nine
// named entries plus generatedPlatforms compact descriptor-only platforms. The
// generated entries prove registry scale and deterministic lookup, not
// platform support — they carry no connector.
func DefaultRegistry(generatedPlatforms int) (*AdapterRegistry, error) {
	if generatedPlatforms < 0 {
		return nil, errors.New("generated platform count cannot be negative")
	}
	r := NewAdapterRegistry()
	named := []AdapterDescriptor{
		{"aurora", "mcp-stdio", StateSupported, []string{"resources", "tools"}, "local stdio contract"},
		{"aurora", "mcp-http", StatePartial, []string{"tools"}, "HTTP/1.1 Content-Length adapter; connectivity is external"},
		{"generic", "rest", StateDescriptorOnly, []string{"request"}, "descriptor only; no live connector"},
		{"generic", "graphql", StateDescriptorOnly, []string{"query"}, "descriptor only; no live connector"},
		{"generic", "webhook", StateDescriptorOnly, []string{"event"}, "descriptor only; no live connector"},
		{"generic", "cli", StateDescriptorOnly, []string{"argv"}, "argv shape only; no process launch"},
		{"generic", "archive", StateDescriptorOnly, []string{"import"}, "archive shape only; no remote fetch"},
		{"generic", "a2a", StateRefused, nil, "wire-shape compatibility is not a live A2A adapter"},
		{"generic", "acp", StateRefused, nil, "ACP is intentionally not implemented"},
	}
	for _, d := range named {
		if err := r.Register(d); err != nil {
			return nil, err
		}
	}
	for i := 0; i < generatedPlatforms; i++ {
		err := r.Register(AdapterDescriptor{
			Platform:     fmt.Sprintf("platform-%04d", i),
			Protocol:     "rest",
			State:        StateDescriptorOnly,
			Capabilities: []string{"request"},
			Notes:        "generated compact descriptor; integration not claimed",
		})
		if err != nil {
			return nil, err
		}
	}
	return r, nil
}

func sortedUnique(caps []string) bool {
	for i := 1; i < len(caps); i++ {
		if caps[i-1] >= caps[i] {
			return false
		}
	}
	return true
}
