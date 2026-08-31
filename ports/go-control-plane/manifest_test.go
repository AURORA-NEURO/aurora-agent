package controlplane

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"testing"
)

func TestFiveMillionLineEquivalentIsLogicalAndBounded(t *testing.T) {
	records, err := SyntheticRecords(1, 5_000_000, 80)
	if err != nil {
		t.Fatal(err)
	}
	chunks, err := ChunksFromRecords(records, 1)
	if err != nil {
		t.Fatal(err)
	}
	summary, err := Summarize("synthetic", chunks)
	if err != nil {
		t.Fatal(err)
	}
	if summary.TotalLines != 5_000_000 || summary.TotalFiles != 1 || summary.ChunkCount != 1 {
		t.Fatalf("summary wrong: %+v", summary)
	}
}

func TestCanonicalRecordLineReproducesCPythonByteForByte(t *testing.T) {
	loadParity(t)
	var vector syntheticRecordVector
	paritySection(t, "synthetic_record", &vector)
	records, _ := SyntheticRecords(2, 100, 80)
	first, ok := records()
	if !ok {
		t.Fatal("synthetic stream produced nothing")
	}
	var buf bytes.Buffer
	canonicalRecordLine(&buf, first)
	if buf.String() != vector.CanonicalLine {
		t.Fatalf("canonical line drifted:\n got %q\nwant %q", buf.String(), vector.CanonicalLine)
	}
	sum := sha256.Sum256(buf.Bytes())
	if hex.EncodeToString(sum[:]) != vector.CanonicalLineSHA {
		t.Fatal("hash of reproduced line does not match the CPython-computed vector")
	}
	if first.Digest != vector.Digest || first.Size != vector.Size || first.Lines != vector.Lines {
		t.Fatalf("record fields drifted: %+v", first)
	}
}

func TestManifestSummaryRootDigestMatchesTheCPythonVector(t *testing.T) {
	loadParity(t)
	var v manifestSummaryVector
	paritySection(t, "manifest_summary", &v)
	records, _ := SyntheticRecords(v.FileCount, v.LinesPerFile, 80)
	chunks, _ := ChunksFromRecords(records, v.ChunkRecords)
	summary, err := Summarize("synthetic", chunks)
	if err != nil {
		t.Fatal(err)
	}
	if summary.RootDigest != v.RootDigest {
		t.Fatalf("root digest drifted:\n got %s\nwant %s", summary.RootDigest, v.RootDigest)
	}
	if summary.TotalFiles != v.TotalFiles || summary.TotalLines != v.TotalLines || summary.ChunkCount != v.ChunkCount {
		t.Fatalf("totals drifted: %+v vs vector %+v", summary, v)
	}
}

func TestChunkStreamIsBoundedToOneChunkOfState(t *testing.T) {
	const fileCount, chunkRecords = 5000, 1024
	records, _ := SyntheticRecords(fileCount, 10, 8)
	chunks, _ := ChunksFromRecords(records, chunkRecords)
	count := 0
	for {
		chunk, ok := chunks()
		if !ok {
			break
		}
		count++
		if len(chunk.Records) > chunkRecords {
			t.Fatalf("chunk %d holds %d records over the bound", chunk.Index, len(chunk.Records))
		}
	}
	if count != (fileCount+chunkRecords-1)/chunkRecords {
		t.Fatalf("chunk count %d wrong for %d files in %d-record chunks",
			count, fileCount, chunkRecords)
	}
	if PeakRecordState(fileCount, chunkRecords) != chunkRecords {
		t.Fatal("peak record state model exceeded one chunk")
	}
}

func TestSyntheticGeometryValidationRejectsBadDimensions(t *testing.T) {
	for _, bad := range [][3]int{
		{-1, 10, 8},
		{10, -1, 8},
		{10, 10, 0},
		{10, 10, -5},
	} {
		if _, err := SyntheticRecords(bad[0], bad[1], bad[2]); err == nil {
			t.Fatalf("geometry %v accepted", bad)
		}
	}
	records, err := SyntheticRecords(0, 10, 8)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := records(); ok {
		t.Fatal("zero-file stream yielded a record")
	}
}

func TestChunkingRefusesNonPositiveChunkRecords(t *testing.T) {
	records, _ := SyntheticRecords(1, 1, 1)
	if _, err := ChunksFromRecords(records, 0); err == nil {
		t.Fatal("chunk_records=0 accepted")
	}
}

// BenchmarkSyntheticManifestMetadata5MLines streams five million logical lines
// through the chunk/summary pipeline per iteration. It creates no files and
// holds at most one chunk of record state; the reported metric is a bound-model
// rate for this metadata pipeline, not a claim about reading real repositories.
func BenchmarkSyntheticManifestMetadata5MLines(b *testing.B) {
	const (
		fileCount     = 5000
		linesPerFile  = 1000
		bytesPerLine  = 80
		chunkRecords  = 1024
		expectedLines = int64(fileCount * linesPerFile)
	)
	records, err := SyntheticRecords(fileCount, linesPerFile, bytesPerLine)
	if err != nil {
		b.Fatal(err)
	}
	chunks, err := ChunksFromRecords(records, chunkRecords)
	if err != nil {
		b.Fatal(err)
	}
	check, err := Summarize("synthetic", chunks)
	if err != nil {
		b.Fatal(err)
	}
	if check.TotalLines != expectedLines {
		b.Fatalf("stream totals wrong: %d", check.TotalLines)
	}

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		records, _ := SyntheticRecords(fileCount, linesPerFile, bytesPerLine)
		chunks, _ := ChunksFromRecords(records, chunkRecords)
		summary, err := Summarize("synthetic", chunks)
		if err != nil || summary.TotalLines != expectedLines {
			b.Fatalf("iteration %d: summary %d err %v", i, summary.TotalLines, err)
		}
	}
	b.StopTimer()
	b.ReportMetric(float64(b.N)*fileCount/b.Elapsed().Seconds(), "records/s")
	b.ReportMetric(float64(b.N)*float64(expectedLines)/b.Elapsed().Seconds(), "lines/s")
	b.Logf("model: %s (peak_record_state=%d)", SyntheticModelDescription,
		PeakRecordState(fileCount, chunkRecords))
}
