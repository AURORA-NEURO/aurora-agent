//! A hand-rolled delimiter-separated reader.
//!
//! There is no CSV crate in this workspace's dependency set and there will not be one: the
//! whole point of 40.17 is that the ingest path is auditable end to end, and a permissive
//! third-party reader would make "what did the source actually say" a question about someone
//! else's heuristics. The parser below is about two hundred lines and every ambiguity in it
//! resolves to a typed error rather than to a repair.
//!
//! What it handles: quoted fields, delimiters and record separators inside quotes, doubled
//! quotes as an escaped quote, LF and CRLF record ends, a UTF-8 byte-order mark, and a final
//! record with or without a trailing newline.
//!
//! What it refuses, by design, because each permissive answer is a guess:
//!
//! - a quote inside an unquoted field (is it data, or did someone forget to quote the field?)
//! - text after a closing quote (`"a"b`)
//! - a carriage return not followed by a line feed (a classic-Mac record end, or literal data?)
//! - a record whose field count differs from the header's (a missing value, or a stray delimiter?)
//! - a duplicate or empty column name (which of the two columns does a mapping rule mean?)
//!
//! Encoding detection is *not* implemented. The reader requires UTF-8 and reports the byte
//! offset where that assumption fails, rather than sniffing a codepage; a mis-sniffed encoding
//! is a silent corruption of every identifier in the file.

use crate::error::CsvError;
use crate::location::SourceLocation;

/// The default field delimiter. Tab-separated sources pass `b'\t'` to [`Table::parse_with`].
pub const COMMA: u8 = b',';

/// A parsed table: a header and zero or more equally wide records.
///
/// Records are one-based in every location this type produces, and the header is *not* record
/// zero — it is addressed with `record: None` — because a loss that applies to a column as a
/// whole is a different claim from a loss in the first data row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    source_id: String,
    headers: Vec<String>,
    records: Vec<Vec<String>>,
}

impl Table {
    /// Parses `bytes` as comma-separated UTF-8 text.
    pub fn parse(source_id: &str, bytes: &[u8]) -> Result<Table, CsvError> {
        Table::parse_with(source_id, bytes, COMMA)
    }

    /// Parses `bytes` with an explicit delimiter.
    pub fn parse_with(source_id: &str, bytes: &[u8], delimiter: u8) -> Result<Table, CsvError> {
        let text = std::str::from_utf8(bytes).map_err(|error| CsvError::NotUtf8 {
            source_id: source_id.to_string(),
            offset: error.valid_up_to(),
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);

        let raw = split_records(source_id, text, delimiter as char)?;
        let mut raw = raw.into_iter();
        let headers = raw.next().ok_or_else(|| CsvError::NoHeader {
            source_id: source_id.to_string(),
        })?;

        for (position, name) in headers.iter().enumerate() {
            if name.trim().is_empty() {
                return Err(CsvError::EmptyColumnName {
                    source_id: source_id.to_string(),
                    position,
                });
            }
            if let Some(first) = headers[..position].iter().position(|other| other == name) {
                return Err(CsvError::DuplicateColumn {
                    source_id: source_id.to_string(),
                    name: name.clone(),
                    first,
                    second: position,
                });
            }
        }

        let mut records = Vec::new();
        for (index, fields) in raw.enumerate() {
            let ordinal = index as u64 + 1;
            if fields.len() != headers.len() {
                return Err(CsvError::ragged(
                    SourceLocation::record(source_id, ordinal),
                    headers.len(),
                    fields.len(),
                ));
            }
            records.push(fields);
        }

        Ok(Table {
            source_id: source_id.to_string(),
            headers,
            records,
        })
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    pub fn records(&self) -> &[Vec<String>] {
        &self.records
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Position of `column` in the header, if present.
    pub fn column_index(&self, column: &str) -> Option<usize> {
        self.headers.iter().position(|name| name == column)
    }

    /// Every value in `column`, in record order.
    pub fn column(&self, column: &str) -> Option<Vec<&str>> {
        let index = self.column_index(column)?;
        Some(
            self.records
                .iter()
                .map(|record| record[index].as_str())
                .collect(),
        )
    }

    /// The one-based location of a cell, for loss entries and provenance.
    pub fn cell_location(&self, record: u64, column: &str) -> SourceLocation {
        SourceLocation::cell(&self.source_id, record, column)
    }

    pub fn column_location(&self, column: &str) -> SourceLocation {
        SourceLocation::column(&self.source_id, column)
    }
}

/// The state machine. Returns raw records; header semantics are applied by the caller.
///
/// `ordinal` counts the record currently being read, with the header as 0, so that a failure
/// inside the first data row reports `record=1` — the same numbering the caller uses after it
/// strips the header, and the same numbering that appears in loss entries and provenance.
///
/// A blank line in the middle of a file becomes a one-field record and is therefore rejected
/// later as ragged, rather than skipped. Skipping it would mean the reader silently disagrees
/// with the byte count of the source about how many records exist.
fn split_records(
    source_id: &str,
    text: &str,
    delimiter: char,
) -> Result<Vec<Vec<String>>, CsvError> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut ordinal: u64 = 0;

    let location = |ordinal: u64| -> SourceLocation {
        if ordinal == 0 {
            SourceLocation::source(source_id)
        } else {
            SourceLocation::record(source_id, ordinal)
        }
    };

    while let Some(ch) = chars.next() {
        if ch == '"' && field.is_empty() {
            loop {
                match chars.next() {
                    None => return Err(CsvError::unterminated_quote(location(ordinal))),
                    Some('"') => {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            field.push('"');
                        } else {
                            break;
                        }
                    }
                    Some(other) => field.push(other),
                }
            }
            match chars.peek().copied() {
                None => {}
                Some(next) if next == delimiter || next == '\n' || next == '\r' => {}
                Some(found) => return Err(CsvError::trailing_garbage(location(ordinal), found)),
            }
            continue;
        }

        if ch == delimiter {
            fields.push(std::mem::take(&mut field));
            continue;
        }

        if ch == '\r' {
            if chars.peek() != Some(&'\n') {
                return Err(CsvError::lone_carriage_return(location(ordinal)));
            }
            chars.next();
            fields.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut fields));
            ordinal += 1;
            continue;
        }

        if ch == '\n' {
            fields.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut fields));
            ordinal += 1;
            continue;
        }

        if ch == '"' {
            return Err(CsvError::bare_quote(location(ordinal)));
        }

        field.push(ch);
    }

    if !field.is_empty() || !fields.is_empty() {
        fields.push(field);
        records.push(fields);
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_field_may_contain_the_delimiter() {
        let table = Table::parse("s", b"a,b\n\"x,y\",z\n").unwrap();
        assert_eq!(table.records()[0], vec!["x,y".to_string(), "z".to_string()]);
    }

    #[test]
    fn a_quoted_field_may_contain_a_record_separator() {
        let table = Table::parse("s", b"a,b\n\"line1\nline2\",z\n").unwrap();
        assert_eq!(table.record_count(), 1);
        assert_eq!(table.records()[0][0], "line1\nline2");
    }

    #[test]
    fn a_doubled_quote_inside_a_quoted_field_is_one_literal_quote() {
        let table = Table::parse("s", b"a\n\"he said \"\"hi\"\"\"\n").unwrap();
        assert_eq!(table.records()[0][0], "he said \"hi\"");
    }

    #[test]
    fn crlf_and_lf_record_ends_parse_to_the_same_table() {
        let crlf = Table::parse("s", b"a,b\r\n1,2\r\n3,4\r\n").unwrap();
        let lf = Table::parse("s", b"a,b\n1,2\n3,4\n").unwrap();
        assert_eq!(crlf.records(), lf.records());
    }

    #[test]
    fn a_final_record_without_a_trailing_newline_is_still_read() {
        let table = Table::parse("s", b"a,b\n1,2").unwrap();
        assert_eq!(table.record_count(), 1);
    }

    #[test]
    fn a_trailing_newline_does_not_create_an_empty_record() {
        let table = Table::parse("s", b"a,b\n1,2\n").unwrap();
        assert_eq!(table.record_count(), 1);
    }

    #[test]
    fn a_utf8_byte_order_mark_is_stripped_from_the_first_column_name() {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(b"subject,age\nS1,40\n");
        let table = Table::parse("s", &bytes).unwrap();
        assert_eq!(table.headers()[0], "subject");
    }

    #[test]
    fn an_unterminated_quote_is_an_error_naming_its_record() {
        let error = Table::parse("s", b"a,b\n\"oops,2\n").unwrap_err();
        assert!(matches!(error, CsvError::UnterminatedQuote { .. }));
    }

    #[test]
    fn a_bare_quote_in_an_unquoted_field_is_an_error_not_data() {
        let error = Table::parse("s", b"a,b\nx\"y,2\n").unwrap_err();
        assert!(matches!(error, CsvError::BareQuote { .. }));
    }

    #[test]
    fn text_after_a_closing_quote_is_an_error() {
        let error = Table::parse("s", b"a,b\n\"x\"y,2\n").unwrap_err();
        assert!(matches!(
            error,
            CsvError::TrailingGarbageAfterQuote { found: 'y', .. }
        ));
    }

    #[test]
    fn a_lone_carriage_return_is_an_error_rather_than_a_guessed_record_end() {
        let error = Table::parse("s", b"a,b\n1,2\r3,4\n").unwrap_err();
        assert!(matches!(error, CsvError::LoneCarriageReturn { .. }));
    }

    #[test]
    fn a_ragged_record_is_an_error_naming_the_record_and_both_widths() {
        let error = Table::parse("s", b"a,b,c\n1,2\n").unwrap_err();
        match error {
            CsvError::RaggedRecord {
                location,
                expected,
                actual,
            } => {
                assert_eq!(location.record, Some(1));
                assert_eq!((expected, actual), (3, 2));
            }
            other => panic!("expected a ragged record error, got {other}"),
        }
    }

    #[test]
    fn a_blank_line_in_the_middle_is_a_ragged_record_rather_than_a_skipped_one() {
        let error = Table::parse("s", b"a,b\n1,2\n\n3,4\n").unwrap_err();
        match error {
            CsvError::RaggedRecord { location, .. } => assert_eq!(location.record, Some(2)),
            other => panic!("expected a ragged record error, got {other}"),
        }
    }

    #[test]
    fn a_tab_delimited_table_parses_when_the_delimiter_is_declared() {
        let table = Table::parse_with("s", b"a\tb\n1\t2\n", b'\t').unwrap();
        assert_eq!(table.headers(), ["a".to_string(), "b".to_string()]);
        assert_eq!(table.records()[0], vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn a_duplicate_column_name_is_an_error_because_a_mapping_could_not_name_one() {
        let error = Table::parse("s", b"a,a\n1,2\n").unwrap_err();
        assert!(matches!(error, CsvError::DuplicateColumn { .. }));
    }

    #[test]
    fn an_empty_column_name_is_an_error() {
        let error = Table::parse("s", b"a,,c\n1,2,3\n").unwrap_err();
        assert!(matches!(
            error,
            CsvError::EmptyColumnName { position: 1, .. }
        ));
    }

    #[test]
    fn empty_input_has_no_header() {
        assert!(matches!(
            Table::parse("s", b"").unwrap_err(),
            CsvError::NoHeader { .. }
        ));
    }

    #[test]
    fn non_utf8_bytes_are_rejected_with_the_offset_rather_than_sniffed() {
        let error = Table::parse("s", &[b'a', b',', b'b', b'\n', 0xff]).unwrap_err();
        assert!(matches!(error, CsvError::NotUtf8 { offset: 4, .. }));
    }

    #[test]
    fn an_empty_quoted_field_is_preserved_as_an_empty_string() {
        let table = Table::parse("s", b"a,b\n\"\",2\n").unwrap();
        assert_eq!(table.records()[0][0], "");
    }

    #[test]
    fn a_header_only_table_has_columns_and_no_records() {
        let table = Table::parse("s", b"a,b\n").unwrap();
        assert_eq!(table.headers().len(), 2);
        assert_eq!(table.record_count(), 0);
    }
}
