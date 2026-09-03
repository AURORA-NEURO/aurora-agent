package controlplane

import (
	"errors"
	"fmt"
	"strings"
	"testing"
)

func TestRequireLiveRefusesDescriptorOnlyAndRefusedStatesWithTheirNotes(t *testing.T) {
	r, err := DefaultRegistry(0)
	if err != nil {
		t.Fatal(err)
	}
	for _, pair := range [][2]string{
		{"generic", "a2a"},
		{"generic", "acp"},
		{"generic", "rest"},
	} {
		_, err := r.RequireLive(pair[0], pair[1])
		var unsupported *UnsupportedAdapterError
		if !errors.As(err, &unsupported) {
			t.Fatalf("%s/%s did not refuse: %v", pair[0], pair[1], err)
		}
		if unsupported.Notes == "" {
			t.Fatalf("%s/%s refused without its notes", pair[0], pair[1])
		}
	}
	d, err := r.RequireLive("aurora", "mcp-stdio")
	if err != nil {
		t.Fatalf("supported surface refused: %v", err)
	}
	if d.State != StateSupported {
		t.Fatalf("aurora/mcp-stdio state %q", d.State)
	}
}

func TestPartialStateIsPresentableAsSupportWhileFlagged(t *testing.T) {
	r, _ := DefaultRegistry(0)
	d, err := r.RequireLive("aurora", "mcp-http")
	if err != nil {
		t.Fatalf("partial surface refused: %v", err)
	}
	if d.State != StatePartial || !strings.Contains(d.Notes, "connectivity is external") {
		t.Fatalf("partial descriptor lost its caveat: %+v", d)
	}
}

func TestUnsortedOrDuplicatedCapabilitiesAreRejectedNotSilentlyFixed(t *testing.T) {
	r := NewAdapterRegistry()
	err := r.Register(AdapterDescriptor{
		Platform: "p", Protocol: "x", State: StateSupported,
		Capabilities: []string{"tools", "resources"},
	})
	var desc *DescriptorError
	if !errors.As(err, &desc) {
		t.Fatalf("unsorted capabilities accepted: %v", err)
	}
	err = r.Register(AdapterDescriptor{
		Platform: "p", Protocol: "y", State: StateSupported,
		Capabilities: []string{"tools", "tools"},
	})
	if !errors.As(err, &desc) {
		t.Fatalf("duplicated capabilities accepted: %v", err)
	}
}

func TestDuplicateRegistrationNamesTheCollisionAndKeepsTheFirst(t *testing.T) {
	r, _ := DefaultRegistry(0)
	err := r.Register(AdapterDescriptor{Platform: "aurora", Protocol: "mcp-stdio", State: StateRefused})
	var desc *DescriptorError
	if !errors.As(err, &desc) || !strings.Contains(desc.Reason, "duplicate") {
		t.Fatalf("duplicate registration: got %v", err)
	}
	d, _ := r.Get("aurora", "mcp-stdio")
	if d.State != StateSupported {
		t.Fatal("collision overwrote the original descriptor")
	}
}

func TestMissingDescriptorIsATypedMissNeverAnEmptyStruct(t *testing.T) {
	r := NewAdapterRegistry()
	_, err := r.Get("ghost", "smtp")
	var unknown *UnknownDescriptorError
	if !errors.As(err, &unknown) {
		t.Fatalf("lookup miss: got %v", err)
	}
}

func TestDefaultRegistryScalesPastOneThousandDescriptorsWithExactLookup(t *testing.T) {
	r, err := DefaultRegistry(1024)
	if err != nil {
		t.Fatal(err)
	}
	if r.Len() < 1000+9 {
		t.Fatalf("registry holds only %d descriptors", r.Len())
	}
	d, err := r.Get("platform-1023", "rest")
	if err != nil || d.State != StateDescriptorOnly {
		t.Fatalf("generated lookup failed: %+v %v", d, err)
	}
	snap := r.Snapshot()
	if len(snap) != r.Len() {
		t.Fatal("snapshot lost entries")
	}
	for i := 1; i < len(snap); i++ {
		prev, cur := snap[i-1], snap[i]
		if prev.Platform > cur.Platform ||
			(prev.Platform == cur.Platform && prev.Protocol >= cur.Protocol) {
			t.Fatalf("snapshot unordered at %d: %s/%s then %s/%s",
				i, prev.Platform, prev.Protocol, cur.Platform, cur.Protocol)
		}
	}
}

func TestNegativeGeneratedPlatformCountIsRejected(t *testing.T) {
	if _, err := DefaultRegistry(-1); err == nil {
		t.Fatal("negative generated count constructed a registry")
	}
}

func TestGeneratedDescriptorsAreShapeOnlyAndNeverClaimConnectivity(t *testing.T) {
	r, _ := DefaultRegistry(4)
	for i := 0; i < 4; i++ {
		if _, err := r.RequireLive(fmt.Sprintf("platform-%04d", i), "rest"); err == nil {
			t.Fatalf("platform-%04d presented as live support", i)
		}
	}
}
