package controlplane

import (
	"encoding/json"
	"os"
	"strconv"
	"testing"
)

// parity holds one decoded section of vectors/parity.json at a time; sections
// are decoded separately so the harness stays independent of vector layout
// above the documented top-level keys.
var parity map[string]json.RawMessage

type mix64Vector struct {
	X string `json:"x"`
	Y string `json:"y"`
}

type homeShardVector struct {
	ShardCount int    `json:"shard_count"`
	Key        string `json:"key"`
	Shard      int    `json:"shard"`
}

type preferenceOrderVector struct {
	ShardCount int    `json:"shard_count"`
	Key        string `json:"key"`
	Order      []int  `json:"order"`
}

type deriveVector struct {
	A   string `json:"a"`
	B   string `json:"b"`
	Hex string `json:"hex"`
}

type assignShardVector struct {
	Key        string `json:"key"`
	ShardCount int    `json:"shard_count"`
	Shard      int    `json:"shard"`
}

type syntheticRecordVector struct {
	Path             string `json:"path"`
	Size             int64  `json:"size"`
	Lines            int64  `json:"lines"`
	Digest           string `json:"digest"`
	CanonicalLine    string `json:"canonical_json_line"`
	CanonicalLineSHA string `json:"canonical_json_line_sha256"`
}

type manifestSummaryVector struct {
	FileCount    int    `json:"file_count"`
	LinesPerFile int    `json:"lines_per_file"`
	ChunkRecords int    `json:"chunk_records"`
	TotalFiles   int    `json:"total_files"`
	TotalLines   int64  `json:"total_lines"`
	ChunkCount   int    `json:"chunk_count"`
	RootDigest   string `json:"root_digest"`
}

func loadParity(t *testing.T) {
	t.Helper()
	if parity != nil {
		return
	}
	data, err := os.ReadFile("vectors/parity.json")
	if err != nil {
		t.Fatalf("parity vectors unreadable: %v", err)
	}
	if err := json.Unmarshal(data, &parity); err != nil {
		t.Fatalf("parity vectors are not valid JSON: %v", err)
	}
}

func paritySection(t *testing.T, key string, out any) {
	t.Helper()
	raw, ok := parity[key]
	if !ok {
		t.Fatalf("parity vectors missing section %q", key)
	}
	if err := json.Unmarshal(raw, out); err != nil {
		t.Fatalf("parity section %q malformed: %v", key, err)
	}
}

func mustU64(t *testing.T, s string) uint64 {
	t.Helper()
	v, err := strconv.ParseUint(s, 10, 64)
	if err != nil {
		t.Fatalf("vector u64 %q: %v", s, err)
	}
	return v
}
