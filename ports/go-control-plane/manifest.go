package controlplane

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"strconv"
	"strings"
)

// SyntheticModelDescription states what the synthetic benchmark measures. It is
// a bound model for record/chunk state, never an OS RSS claim and never a file
// count: no files are created at any point.
const SyntheticModelDescription = "synthetic-logical-records; no files created"

// FileRecord is one bounded metadata record: where a file sits, how big it is,
// how many lines it carries, and the digest of its bytes.
type FileRecord struct {
	Path   string
	Size   int64
	Lines  int64
	Digest string
}

// Chunk is a deterministic batch of records. Its digest covers the exact
// canonical bytes of its records, so equal chunks are byte-equal across
// languages and runs.
type Chunk struct {
	Index   int
	Records []FileRecord
	Digest  string
	Bytes   int64
	Lines   int64
}

// ManifestSummary folds chunk digests into one root digest plus totals.
type ManifestSummary struct {
	Root       string
	TotalFiles int
	TotalBytes int64
	TotalLines int64
	ChunkCount int
	RootDigest string
}

// RecordStream is a pull iterator over records. Streaming keeps memory bounded:
// a five-million-line repository is a scale target, not a requirement to hold
// five million records in memory.
type RecordStream func() (FileRecord, bool)

// ChunkStream is a pull iterator over chunks.
type ChunkStream func() (*Chunk, bool)

// SyntheticRecords yields logical records modelling a repository of the given
// shape without creating files or line-sized buffers. Each record's digest is
// explicitly a logical digest of its declared geometry, not of any file bytes.
func SyntheticRecords(fileCount, linesPerFile, bytesPerLine int) (RecordStream, error) {
	if fileCount < 0 || linesPerFile < 0 || bytesPerLine <= 0 {
		return nil, errors.New("synthetic dimensions must be nonnegative with positive bytes_per_line")
	}
	next := 0
	return func() (FileRecord, bool) {
		if next >= fileCount {
			return FileRecord{}, false
		}
		path := fmt.Sprintf("synthetic/module-%08d.src", next)
		logical := fmt.Sprintf("synthetic:%s:%d:%d", path, linesPerFile, bytesPerLine)
		sum := sha256.Sum256([]byte(logical))
		next++
		return FileRecord{
			Path:   path,
			Size:   int64(linesPerFile) * int64(bytesPerLine),
			Lines:  int64(linesPerFile),
			Digest: hex.EncodeToString(sum[:]),
		}, true
	}, nil
}

// ChunksFromRecords batches a record stream into bounded chunks whose digests
// are stable across runs with the same bytes and paths.
func ChunksFromRecords(records RecordStream, chunkRecords int) (ChunkStream, error) {
	if chunkRecords <= 0 {
		return nil, errors.New("chunk_records must be positive")
	}
	index := 0
	return func() (*Chunk, bool) {
		bucket := make([]FileRecord, 0, chunkRecords)
		var lineBuf strings.Builder
		for len(bucket) < chunkRecords {
			rec, ok := records()
			if !ok {
				break
			}
			bucket = append(bucket, rec)
			canonicalRecordLine(&lineBuf, rec)
		}
		if len(bucket) == 0 {
			return nil, false
		}
		sum := sha256.Sum256([]byte(lineBuf.String()))
		var bytes, lines int64
		for _, r := range bucket {
			bytes += r.Size
			lines += r.Lines
		}
		chunk := &Chunk{
			Index:   index,
			Records: bucket,
			Digest:  hex.EncodeToString(sum[:]),
			Bytes:   bytes,
			Lines:   lines,
		}
		index++
		return chunk, true
	}, nil
}

// Summarize folds a chunk stream into totals and the root digest: sha256 over
// the concatenated raw chunk-digest bytes, matching the Python scale layer.
func Summarize(root string, chunks ChunkStream) (ManifestSummary, error) {
	rootHash := sha256.New()
	var summary ManifestSummary
	summary.Root = root
	for {
		chunk, ok := chunks()
		if !ok {
			break
		}
		raw, err := hex.DecodeString(chunk.Digest)
		if err != nil {
			return ManifestSummary{}, fmt.Errorf("chunk %d digest is not valid hex: %w", chunk.Index, err)
		}
		rootHash.Write(raw)
		summary.TotalFiles += len(chunk.Records)
		summary.TotalBytes += chunk.Bytes
		summary.TotalLines += chunk.Lines
		summary.ChunkCount++
	}
	summary.RootDigest = hex.EncodeToString(rootHash.Sum(nil))
	return summary, nil
}

// PeakRecordState is the bound-model peak number of records held at once:
// never the whole stream, always at most one chunk.
func PeakRecordState(fileCount, chunkRecords int) int {
	if fileCount < chunkRecords {
		return fileCount
	}
	return chunkRecords
}

// canonicalRecordLine appends the exact bytes CPython's
// json.dumps(asdict(record), sort_keys=True, separators=(",", ":"),
// ensure_ascii=False) + "\n" produces. Hand-rolled because encoding/json
// escapes HTML-significant characters and U+2028/U+2029, which CPython does
// not; parity here means matching the reference encoder, not Go's default.
func canonicalRecordLine(w *strings.Builder, r FileRecord) {
	w.WriteString(`{"digest":"`)
	w.WriteString(r.Digest)
	w.WriteString(`","lines":`)
	w.WriteString(strconv.FormatInt(r.Lines, 10))
	w.WriteString(`,"path":"`)
	canonicalJSONString(w, r.Path)
	w.WriteString(`","size":`)
	w.WriteString(strconv.FormatInt(r.Size, 10))
	w.WriteString("}\n")
}

func canonicalJSONString(w *strings.Builder, s string) {
	for _, r := range s {
		switch {
		case r == '"':
			w.WriteString(`\"`)
		case r == '\\':
			w.WriteString(`\\`)
		case r == '\b':
			w.WriteString(`\b`)
		case r == '\f':
			w.WriteString(`\f`)
		case r == '\n':
			w.WriteString(`\n`)
		case r == '\r':
			w.WriteString(`\r`)
		case r == '\t':
			w.WriteString(`\t`)
		case r < 0x20:
			const hexDigits = "0123456789abcdef"
			w.WriteString(`\u00`)
			w.WriteByte(hexDigits[(r>>4)&0xF])
			w.WriteByte(hexDigits[r&0xF])
		default:
			w.WriteRune(r)
		}
	}
}
