//! An on-disk sorted map with binary-search lookup.
//!
//! Two files per index: `<name>.keys` holds `key\tvalue` records sorted by key, one per line, and
//! `<name>.offs` holds one little-endian `u64` byte offset per record. A lookup binary-searches
//! the offset table, seeking and reading a single record per probe — `O(log n)` small reads
//! rather than a linear scan or a whole-file parse.
//!
//! The point is that the *index itself* must not be loaded eagerly. A `HashMap` of a million fact
//! offsets costs as much to build as parsing the world did, which would leave the compiler exactly
//! as input-sensitive as before.

use crate::error::StoreError;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub struct SortedIndexWriter {
    records: Vec<(String, String)>,
}

impl SortedIndexWriter {
    pub fn new() -> Self {
        SortedIndexWriter {
            records: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.records.push((key.into(), value.into()));
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Writes the index. Later duplicates win, matching the reference runtime's dict semantics.
    pub fn finish(mut self, directory: &Path, name: &str) -> Result<(), StoreError> {
        for (key, value) in &self.records {
            if key.contains('\t') || key.contains('\n') {
                return Err(StoreError::UnsupportedKey(key.clone()));
            }
            if value.contains('\n') {
                return Err(StoreError::UnsupportedValue(key.clone()));
            }
        }

        self.records.sort_by(|a, b| a.0.cmp(&b.0));
        self.records.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                earlier.1 = later.1.clone();
                true
            } else {
                false
            }
        });

        let keys_path = directory.join(format!("{name}.keys"));
        let offs_path = directory.join(format!("{name}.offs"));
        let mut keys = BufWriter::new(File::create(&keys_path)?);
        let mut offs = BufWriter::new(File::create(&offs_path)?);

        let mut cursor: u64 = 0;
        for (key, value) in &self.records {
            offs.write_all(&cursor.to_le_bytes())?;
            let line = format!("{key}\t{value}\n");
            keys.write_all(line.as_bytes())?;
            cursor += line.len() as u64;
        }
        keys.flush()?;
        offs.flush()?;
        Ok(())
    }
}

impl Default for SortedIndexWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SortedIndex {
    keys_path: PathBuf,
    offsets: Vec<u64>,
}

impl SortedIndex {
    /// Opens an index.
    ///
    /// The offset table is read into memory — 8 bytes per record, so 8 MB for a million facts —
    /// while the records themselves stay on disk. This is the deliberate trade: a small dense
    /// array bought with one sequential read, against per-probe seeks into a large file.
    pub fn open(directory: &Path, name: &str) -> Result<Self, StoreError> {
        let keys_path = directory.join(format!("{name}.keys"));
        let offs_path = directory.join(format!("{name}.offs"));

        let mut raw = Vec::new();
        File::open(&offs_path)?.read_to_end(&mut raw)?;
        if raw.len() % 8 != 0 {
            return Err(StoreError::CorruptIndex(name.to_string()));
        }
        let offsets = raw
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("chunk is 8 bytes")))
            .collect();

        Ok(SortedIndex { keys_path, offsets })
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        if self.offsets.is_empty() {
            return Ok(None);
        }
        let mut file = File::open(&self.keys_path)?;

        let mut low = 0usize;
        let mut high = self.offsets.len();
        while low < high {
            let middle = (low + high) / 2;
            let (found_key, value) = self.read_record(&mut file, middle)?;
            match found_key.as_str().cmp(key) {
                std::cmp::Ordering::Equal => return Ok(Some(value)),
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
            }
        }
        Ok(None)
    }

    fn read_record(&self, file: &mut File, position: usize) -> Result<(String, String), StoreError> {
        let start = self.offsets[position];
        let end = self
            .offsets
            .get(position + 1)
            .copied()
            .unwrap_or_else(|| file.metadata().map(|m| m.len()).unwrap_or(start));
        let length = (end - start) as usize;

        file.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0u8; length];
        file.read_exact(&mut buffer)?;

        let line = String::from_utf8(buffer).map_err(|_| StoreError::CorruptIndex("utf8".into()))?;
        let line = line.trim_end_matches('\n');
        let (key, value) = line
            .split_once('\t')
            .ok_or_else(|| StoreError::CorruptIndex("missing separator".into()))?;
        Ok((key.to_string(), value.to_string()))
    }
}
