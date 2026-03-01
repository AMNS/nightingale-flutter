//! Notelist parser implementation.
//!
//! Parses Notelist V1 and V2 text files into structured data.

use std::fmt;
use std::io::Read;

/// Parse error type.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Invalid header line.
    InvalidHeader(String),
    /// Missing required field in record.
    MissingField { line_num: usize, field: String },
    /// Invalid field value.
    InvalidValue {
        line_num: usize,
        field: String,
        value: String,
    },
    /// Unknown record type.
    UnknownRecordType { line_num: usize, record_type: char },
    /// I/O error.
    IoError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidHeader(msg) => write!(f, "Invalid header: {}", msg),
            ParseError::MissingField { line_num, field } => {
                write!(f, "Line {}: missing field '{}'", line_num, field)
            }
            ParseError::InvalidValue {
                line_num,
                field,
                value,
            } => write!(
                f,
                "Line {}: invalid value '{}' for field '{}'",
                line_num, value, field
            ),
            ParseError::UnknownRecordType {
                line_num,
                record_type,
            } => write!(
                f,
                "Line {}: unknown record type '{}'",
                line_num, record_type
            ),
            ParseError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        ParseError::IoError(err.to_string())
    }
}

/// A parsed Notelist file.
#[derive(Debug, Clone, PartialEq)]
pub struct Notelist {
    /// Notelist version (1 or 2).
    pub version: u8,
    /// Original filename from header.
    pub filename: String,
    /// Staves per part (e.g., [2, 0] means part 1 has 2 staves, part 2 has 0).
    pub part_staves: Vec<u8>,
    /// Starting measure number.
    pub start_meas: i32,
    /// All records in the file.
    pub records: Vec<NotelistRecord>,
}

/// A single record in a Notelist file.
#[derive(Debug, Clone, PartialEq)]
pub enum NotelistRecord {
    /// Note record (N).
    Note {
        time: i32,
        voice: i8,
        part: i8,
        staff: i8,
        dur: i8,
        dots: u8,
        note_num: u8,
        acc: u8,
        effective_acc: u8,
        play_dur: i16,
        velocity: u8,
        stem_info: String,
        appear: u8,
        mods: Option<u8>,
    },
    /// Rest record (R).
    Rest {
        time: i32,
        voice: i8,
        part: i8,
        staff: i8,
        dur: i8,
        dots: u8,
        stem_info: String,
        appear: u8,
        mods: Option<u8>,
    },
    /// Grace note record (G).
    GraceNote {
        voice: i8,
        part: i8,
        staff: i8,
        dur: i8,
        dots: u8,
        note_num: u8,
        acc: u8,
        effective_acc: u8,
        play_dur: i16,
        velocity: u8,
        stem_char: char,
        appear: u8,
        mods: Option<u8>,
    },
    /// Barline record (/).
    Barline {
        time: i32,
        bar_type: u8,
        number: Option<i32>,
    },
    /// Clef record (C).
    Clef { staff: i8, clef_type: u8 },
    /// Key signature record (K).
    KeySig {
        staff: i8,
        n_items: u8,
        is_sharp: bool,
    },
    /// Time signature record (T).
    TimeSig {
        staff: i8,
        numerator: i8,
        denominator: i8,
    },
    /// Dynamic record (D).
    Dynamic { staff: i8, dynamic_type: u8 },
    /// Text record (A).
    Text {
        voice: i8,
        part: i8,
        staff: i8,
        style: String,
        text: String,
    },
    /// Tempo record (M).
    Tempo {
        staff: i8,
        tempo_str: String,
        beat_char: char,
        dotted: bool,
        mm_str: String,
    },
    /// Tuplet record (P).
    Tuplet {
        voice: i8,
        part: i8,
        num: u8,
        denom: u8,
        appear: String,
    },
    /// Beam record (B).
    Beam { voice: i8, part: i8, count: u8 },
    /// Comment record (%).
    Comment(String),
}

/// Parse a Notelist file from a reader.
pub fn parse_notelist<R: Read>(reader: R) -> Result<Notelist, ParseError> {
    // Read the entire file as bytes first, then convert to lossy UTF-8.
    // This handles files with non-UTF-8 characters (e.g., copyright symbols).
    let mut bytes = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut bytes)?;
    let content = String::from_utf8_lossy(&bytes);

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Err(ParseError::InvalidHeader("Empty file".to_string()));
    }

    // Parse header line.
    let (version, filename, part_staves, start_meas) = parse_header(lines[0], 1)?;

    let mut records = Vec::new();

    // Read records.
    for (idx, line) in lines.iter().skip(1).enumerate() {
        let line_num = idx + 2; // +2 because we skipped header and indices are 0-based
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record = parse_record(trimmed, line_num)?;
        records.push(record);
    }

    Ok(Notelist {
        version,
        filename,
        part_staves,
        start_meas,
        records,
    })
}

/// Parse the header line.
fn parse_header(line: &str, line_num: usize) -> Result<(u8, String, Vec<u8>, i32), ParseError> {
    let line = line.trim();

    // Expected format: %%Notelist-V2 file='...' partstaves=<n> <n>... startmeas=<n>
    if !line.starts_with("%%Notelist-V") {
        return Err(ParseError::InvalidHeader(
            "Missing %%Notelist-V prefix".to_string(),
        ));
    }

    // Extract version.
    let version = if line.starts_with("%%Notelist-V2") {
        2
    } else if line.starts_with("%%Notelist-V1") {
        1
    } else {
        return Err(ParseError::InvalidHeader(format!(
            "Unknown version: {}",
            &line[0..20.min(line.len())]
        )));
    };

    // Parse file='...' by finding the quoted string after file=
    let filename = if let Some(file_start) = line.find("file='") {
        let after_equals = file_start + 6; // Skip "file='"
        if let Some(quote_end) = line[after_equals..].find('\'') {
            line[after_equals..after_equals + quote_end].to_string()
        } else {
            return Err(ParseError::InvalidHeader(
                "Unterminated file name quote".to_string(),
            ));
        }
    } else {
        return Err(ParseError::MissingField {
            line_num,
            field: "file".to_string(),
        });
    };

    let tokens: Vec<&str> = line.split_whitespace().collect();

    // Parse partstaves=<n> <n>...
    // Format: partstaves=<value1> <value2> ... startmeas=...
    // The first value after "partstaves=" is part of the array, not a count.
    let partstaves_idx = tokens
        .iter()
        .position(|t| t.starts_with("partstaves="))
        .ok_or_else(|| ParseError::MissingField {
            line_num,
            field: "partstaves".to_string(),
        })?;

    let first_val_str = tokens[partstaves_idx].strip_prefix("partstaves=").unwrap();
    let first_val = first_val_str
        .parse::<u8>()
        .map_err(|_| ParseError::InvalidValue {
            line_num,
            field: "partstaves".to_string(),
            value: first_val_str.to_string(),
        })?;

    let mut part_staves = vec![first_val];

    // Collect remaining values until we hit startmeas=
    for i in 1.. {
        if partstaves_idx + i >= tokens.len() {
            break;
        }
        let token = tokens[partstaves_idx + i];
        if token.starts_with("startmeas=") {
            break;
        }
        let val = token.parse::<u8>().map_err(|_| ParseError::InvalidValue {
            line_num,
            field: "partstaves entry".to_string(),
            value: token.to_string(),
        })?;
        part_staves.push(val);
    }

    // Parse startmeas=<n>
    let start_meas = tokens
        .iter()
        .find(|t| t.starts_with("startmeas="))
        .and_then(|t| t.strip_prefix("startmeas="))
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| ParseError::MissingField {
            line_num,
            field: "startmeas".to_string(),
        })?;

    Ok((version, filename, part_staves, start_meas))
}

/// Parse a single record line.
fn parse_record(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    if line.is_empty() {
        return Err(ParseError::InvalidValue {
            line_num,
            field: "record".to_string(),
            value: "(empty line)".to_string(),
        });
    }

    let first_char = line.chars().next().unwrap();

    match first_char {
        'N' => parse_note(line, line_num),
        'R' => parse_rest(line, line_num),
        'G' => parse_grace_note(line, line_num),
        '/' => parse_barline(line, line_num),
        'C' => parse_clef(line, line_num),
        'K' => parse_key_sig(line, line_num),
        'T' => parse_time_sig(line, line_num),
        'D' => parse_dynamic(line, line_num),
        'A' => parse_text(line, line_num),
        'M' => parse_tempo(line, line_num),
        'P' => parse_tuplet(line, line_num),
        'B' => parse_beam(line, line_num),
        '%' => Ok(NotelistRecord::Comment(line[1..].trim().to_string())),
        _ => Err(ParseError::UnknownRecordType {
            line_num,
            record_type: first_char,
        }),
    }
}

/// Parse a key=value pair from a token.
fn parse_kv(token: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = token.splitn(2, '=').collect();
    if parts.len() == 2 {
        Some((parts[0], parts[1]))
    } else {
        None
    }
}

/// Extract fields from a line into a key-value map.
fn extract_fields(
    line: &str,
    _line_num: usize,
) -> Result<std::collections::HashMap<String, String>, ParseError> {
    let mut fields = std::collections::HashMap::new();

    // Split on whitespace but preserve quoted strings.
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '\'' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // Skip first token (record type).
    for token in tokens.iter().skip(1) {
        if let Some((key, val)) = parse_kv(token) {
            fields.insert(key.to_string(), val.to_string());
        } else if !token.starts_with('\'') {
            // Not a quoted string and not a key=value pair - might be a standalone value.
            // For stem info and other fields without keys.
            fields.insert("_standalone".to_string(), token.to_string());
        }
    }

    Ok(fields)
}

/// Get a required field value.
fn get_field<'a>(
    fields: &'a std::collections::HashMap<String, String>,
    key: &str,
    line_num: usize,
) -> Result<&'a str, ParseError> {
    fields
        .get(key)
        .map(|s| s.as_str())
        .ok_or_else(|| ParseError::MissingField {
            line_num,
            field: key.to_string(),
        })
}

/// Parse an integer field.
fn parse_i32_field(
    fields: &std::collections::HashMap<String, String>,
    key: &str,
    line_num: usize,
) -> Result<i32, ParseError> {
    let val = get_field(fields, key, line_num)?;
    val.parse::<i32>().map_err(|_| ParseError::InvalidValue {
        line_num,
        field: key.to_string(),
        value: val.to_string(),
    })
}

fn parse_i8_field(
    fields: &std::collections::HashMap<String, String>,
    key: &str,
    line_num: usize,
) -> Result<i8, ParseError> {
    let val = get_field(fields, key, line_num)?;
    val.parse::<i8>().map_err(|_| ParseError::InvalidValue {
        line_num,
        field: key.to_string(),
        value: val.to_string(),
    })
}

fn parse_u8_field(
    fields: &std::collections::HashMap<String, String>,
    key: &str,
    line_num: usize,
) -> Result<u8, ParseError> {
    let val = get_field(fields, key, line_num)?;
    val.parse::<u8>().map_err(|_| ParseError::InvalidValue {
        line_num,
        field: key.to_string(),
        value: val.to_string(),
    })
}

fn parse_i16_field(
    fields: &std::collections::HashMap<String, String>,
    key: &str,
    line_num: usize,
) -> Result<i16, ParseError> {
    let val = get_field(fields, key, line_num)?;
    val.parse::<i16>().map_err(|_| ParseError::InvalidValue {
        line_num,
        field: key.to_string(),
        value: val.to_string(),
    })
}

/// Parse note record.
fn parse_note(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    // Extract stem info (6 chars like "......" or "+....." or "-.....").
    // It appears after the velocity and before appear=.
    let stem_info = find_stem_info(line, 6);

    Ok(NotelistRecord::Note {
        time: parse_i32_field(&fields, "t", line_num)?,
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        staff: parse_i8_field(&fields, "stf", line_num)?,
        dur: parse_i8_field(&fields, "dur", line_num)?,
        dots: parse_u8_field(&fields, "dots", line_num)?,
        note_num: parse_u8_field(&fields, "nn", line_num)?,
        acc: parse_u8_field(&fields, "acc", line_num)?,
        effective_acc: parse_u8_field(&fields, "eAcc", line_num)?,
        play_dur: parse_i16_field(&fields, "pDur", line_num)?,
        velocity: parse_u8_field(&fields, "vel", line_num)?,
        stem_info,
        appear: parse_u8_field(&fields, "appear", line_num)?,
        mods: fields.get("mods").and_then(|s| s.parse::<u8>().ok()),
    })
}

/// Parse rest record.
fn parse_rest(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;
    let stem_info = find_stem_info(line, 6);

    Ok(NotelistRecord::Rest {
        time: parse_i32_field(&fields, "t", line_num)?,
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        staff: parse_i8_field(&fields, "stf", line_num)?,
        dur: parse_i8_field(&fields, "dur", line_num)?,
        dots: parse_u8_field(&fields, "dots", line_num)?,
        stem_info,
        appear: parse_u8_field(&fields, "appear", line_num)?,
        mods: fields.get("mods").and_then(|s| s.parse::<u8>().ok()),
    })
}

/// Parse grace note record.
fn parse_grace_note(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    // For grace notes, stem info is a single char like '.' or '+' or '-'.
    let stem_char = find_stem_info(line, 1).chars().next().unwrap_or('.');

    Ok(NotelistRecord::GraceNote {
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        staff: parse_i8_field(&fields, "stf", line_num)?,
        dur: parse_i8_field(&fields, "dur", line_num)?,
        dots: parse_u8_field(&fields, "dots", line_num)?,
        note_num: parse_u8_field(&fields, "nn", line_num)?,
        acc: parse_u8_field(&fields, "acc", line_num)?,
        effective_acc: parse_u8_field(&fields, "eAcc", line_num)?,
        play_dur: parse_i16_field(&fields, "pDur", line_num)?,
        velocity: parse_u8_field(&fields, "vel", line_num)?,
        stem_char,
        appear: parse_u8_field(&fields, "appear", line_num)?,
        mods: fields.get("mods").and_then(|s| s.parse::<u8>().ok()),
    })
}

/// Parse barline record.
fn parse_barline(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    Ok(NotelistRecord::Barline {
        time: parse_i32_field(&fields, "t", line_num)?,
        bar_type: parse_u8_field(&fields, "type", line_num)?,
        number: fields.get("number").and_then(|s| s.parse::<i32>().ok()),
    })
}

/// Parse clef record.
fn parse_clef(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    Ok(NotelistRecord::Clef {
        staff: parse_i8_field(&fields, "stf", line_num)?,
        clef_type: parse_u8_field(&fields, "type", line_num)?,
    })
}

/// Parse key signature record.
fn parse_key_sig(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    // Determine if sharp or flat by looking for '#' or 'b' in the line.
    let is_sharp = line.contains(" #") || line.ends_with('#');

    Ok(NotelistRecord::KeySig {
        staff: parse_i8_field(&fields, "stf", line_num)?,
        n_items: parse_u8_field(&fields, "KS", line_num)?,
        is_sharp,
    })
}

/// Parse time signature record.
fn parse_time_sig(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    Ok(NotelistRecord::TimeSig {
        staff: parse_i8_field(&fields, "stf", line_num)?,
        numerator: parse_i8_field(&fields, "num", line_num)?,
        denominator: parse_i8_field(&fields, "denom", line_num)?,
    })
}

/// Parse dynamic record.
fn parse_dynamic(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    Ok(NotelistRecord::Dynamic {
        staff: parse_i8_field(&fields, "stf", line_num)?,
        dynamic_type: parse_u8_field(&fields, "dType", line_num)?,
    })
}

/// Parse text record (A).
/// Format: A v=<voice> npt=<part> stf=<staff> <stylecode> '<text>'
fn parse_text(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    // Extract style code (e.g., S0, L1, S3).
    // It's the token before the quoted string.
    let style = extract_style_code(line);

    // Extract quoted text.
    let text = extract_quoted_text(line);

    Ok(NotelistRecord::Text {
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        staff: parse_i8_field(&fields, "stf", line_num)?,
        style,
        text,
    })
}

/// Parse tempo record (M).
/// Format: M stf=<staff> '<tempostr>' <beatchar>[.]= <mmstr>
fn parse_tempo(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    let tempo_str = extract_quoted_text(line);

    // Extract beat char and whether it's dotted.
    // Look for pattern like "q=" or "q.=" or "h=" etc.
    let (beat_char, dotted, mm_str) = extract_beat_and_mm(line);

    Ok(NotelistRecord::Tempo {
        staff: parse_i8_field(&fields, "stf", line_num)?,
        tempo_str,
        beat_char,
        dotted,
        mm_str,
    })
}

/// Parse tuplet record (P).
/// Format: P v=<voice> npt=<part> num=<num> denom=<denom> appear=<appearcode>
fn parse_tuplet(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    let appear = get_field(&fields, "appear", line_num)?.to_string();

    Ok(NotelistRecord::Tuplet {
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        num: parse_u8_field(&fields, "num", line_num)?,
        denom: parse_u8_field(&fields, "denom", line_num)?,
        appear,
    })
}

/// Parse beam record (B).
/// Format: B v=<voice> npt=<part> count=<n>
fn parse_beam(line: &str, line_num: usize) -> Result<NotelistRecord, ParseError> {
    let fields = extract_fields(line, line_num)?;

    Ok(NotelistRecord::Beam {
        voice: parse_i8_field(&fields, "v", line_num)?,
        part: parse_i8_field(&fields, "npt", line_num)?,
        count: parse_u8_field(&fields, "count", line_num)?,
    })
}

/// Find stem info in a line — the 6-character note flag string.
///
/// Format (from NotelistSave.cp:130-136):
///   pos 0: mCode — '.' (standalone), '+' (chord main), '-' (chord secondary)
///   pos 1: tiedL — ')' if tied to left, else '.'
///   pos 2: tiedR — '(' if tied to right, else '.'
///   pos 3: slurredL — '>' if slurred to left, else '.'
///   pos 4: slurredR — '<' if slurred to right, else '.'
///   pos 5: inTuplet — 'T' if in tuplet, else '.'
fn find_stem_info(line: &str, expected_len: usize) -> String {
    // Valid characters in the stem_info / note-flag string.
    const VALID_CHARS: &[char] = &['.', '+', '-', ')', '(', '>', '<', 'T'];

    let tokens: Vec<&str> = line.split_whitespace().collect();

    for token in tokens {
        if token.len() == expected_len && token.chars().all(|c| VALID_CHARS.contains(&c)) {
            return token.to_string();
        }
    }

    // Default to dots.
    ".".repeat(expected_len)
}

/// Extract style code from text record.
fn extract_style_code(line: &str) -> String {
    // Style code appears after stf=<n> and before the quoted string.
    // It's typically S0, L1, S3, etc.
    let tokens: Vec<&str> = line.split_whitespace().collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.starts_with('\'') {
            // Previous token should be the style code.
            if i > 0 {
                let prev = tokens[i - 1];
                if !prev.contains('=') {
                    return prev.to_string();
                }
            }
            break;
        }
    }

    String::new()
}

/// Extract quoted text from a line.
fn extract_quoted_text(line: &str) -> String {
    // Find text between single quotes.
    if let Some(start) = line.find('\'') {
        if let Some(end) = line[start + 1..].rfind('\'') {
            return line[start + 1..start + 1 + end].to_string();
        }
    }
    String::new()
}

/// Extract beat character, dotted flag, and MM string from tempo line.
fn extract_beat_and_mm(line: &str) -> (char, bool, String) {
    // Look for pattern like "q=" or "q.=" after the quoted string.
    let tokens: Vec<&str> = line.split_whitespace().collect();

    let mut found_quote_end = false;
    for token in tokens {
        if found_quote_end && token.contains('=') {
            // This should be the beat char pattern like "q=" or "q.=".
            let parts: Vec<&str> = token.split('=').collect();
            if let Some(beat_part) = parts.first() {
                let dotted = beat_part.contains('.');
                let beat_char = beat_part.chars().next().unwrap_or('q');
                let mm_str = parts.get(1).unwrap_or(&"").to_string();
                return (beat_char, dotted, mm_str);
            }
        }
        if token.ends_with('\'') {
            found_quote_end = true;
        }
    }

    ('q', false, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_v2() {
        let line = "%%Notelist-V2 file='HBD_33 (converted)'  partstaves=2 0 startmeas=0";
        let (version, filename, part_staves, start_meas) = parse_header(line, 1).unwrap();

        assert_eq!(version, 2);
        assert_eq!(filename, "HBD_33 (converted)");
        assert_eq!(part_staves, vec![2, 0]);
        assert_eq!(start_meas, 0);
    }

    #[test]
    fn test_parse_note() {
        let line =
            "N t=0 v=1 npt=1 stf=1 dur=5 dots=0 nn=67 acc=0 eAcc=3 pDur=240 vel=75 ...... appear=1";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::Note {
                time,
                voice,
                part,
                staff,
                dur,
                dots,
                note_num,
                acc,
                effective_acc,
                play_dur,
                velocity,
                stem_info,
                appear,
                mods,
            } => {
                assert_eq!(time, 0);
                assert_eq!(voice, 1);
                assert_eq!(part, 1);
                assert_eq!(staff, 1);
                assert_eq!(dur, 5);
                assert_eq!(dots, 0);
                assert_eq!(note_num, 67);
                assert_eq!(acc, 0);
                assert_eq!(effective_acc, 3);
                assert_eq!(play_dur, 240);
                assert_eq!(velocity, 75);
                assert_eq!(stem_info, "......");
                assert_eq!(appear, 1);
                assert_eq!(mods, None);
            }
            _ => panic!("Expected Note record"),
        }
    }

    #[test]
    fn test_parse_barline() {
        let line = "/ t=480 type=1";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::Barline {
                time,
                bar_type,
                number,
            } => {
                assert_eq!(time, 480);
                assert_eq!(bar_type, 1);
                assert_eq!(number, None);
            }
            _ => panic!("Expected Barline record"),
        }
    }

    #[test]
    fn test_parse_clef() {
        let line = "C stf=1 type=3";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::Clef { staff, clef_type } => {
                assert_eq!(staff, 1);
                assert_eq!(clef_type, 3);
            }
            _ => panic!("Expected Clef record"),
        }
    }

    #[test]
    fn test_parse_key_sig() {
        let line = "K stf=1 KS=0 #";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::KeySig {
                staff,
                n_items,
                is_sharp,
            } => {
                assert_eq!(staff, 1);
                assert_eq!(n_items, 0);
                assert!(is_sharp);
            }
            _ => panic!("Expected KeySig record"),
        }
    }

    #[test]
    fn test_parse_time_sig() {
        let line = "T stf=1 num=3 denom=4";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::TimeSig {
                staff,
                numerator,
                denominator,
            } => {
                assert_eq!(staff, 1);
                assert_eq!(numerator, 3);
                assert_eq!(denominator, 4);
            }
            _ => panic!("Expected TimeSig record"),
        }
    }

    #[test]
    fn test_parse_text() {
        let line = "A v=-2 npt=-2 stf=-2 S0 'Hippo Birdy; Two Ewes.'";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::Text {
                voice,
                part,
                staff,
                style,
                text,
            } => {
                assert_eq!(voice, -2);
                assert_eq!(part, -2);
                assert_eq!(staff, -2);
                assert_eq!(style, "S0");
                assert_eq!(text, "Hippo Birdy; Two Ewes.");
            }
            _ => panic!("Expected Text record"),
        }
    }

    #[test]
    fn test_parse_comment() {
        let line = "% This is a comment";
        let record = parse_record(line, 1).unwrap();

        match record {
            NotelistRecord::Comment(text) => {
                assert_eq!(text, "This is a comment");
            }
            _ => panic!("Expected Comment record"),
        }
    }

    #[test]
    fn test_parse_hbd_33() {
        let file = std::fs::File::open("tests/notelist_examples/HBD_33.nl").unwrap();
        let notelist = parse_notelist(file).unwrap();

        assert_eq!(notelist.version, 2);
        assert_eq!(notelist.filename, "HBD_33 (converted)");
        assert_eq!(notelist.part_staves, vec![2, 0]);
        assert_eq!(notelist.start_meas, 0);

        // Count record types.
        let mut note_count = 0;
        let mut barline_count = 0;
        let mut clef_count = 0;
        let mut text_count = 0;

        for record in &notelist.records {
            match record {
                NotelistRecord::Note { .. } => note_count += 1,
                NotelistRecord::Barline { .. } => barline_count += 1,
                NotelistRecord::Clef { .. } => clef_count += 1,
                NotelistRecord::Text { .. } => text_count += 1,
                _ => {}
            }
        }

        // Basic sanity checks.
        assert!(note_count > 0, "Should have notes");
        assert!(barline_count > 0, "Should have barlines");
        assert!(clef_count > 0, "Should have clefs");
        assert!(text_count > 0, "Should have text records");
    }

    #[test]
    fn test_parse_all_examples() {
        // Parse all .nl files in tests/notelist_examples/ without panicking.
        let paths = std::fs::read_dir("tests/notelist_examples").unwrap();

        let mut count = 0;
        for path in paths {
            let path = path.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) == Some("nl") {
                let file = std::fs::File::open(&path).unwrap();
                let result = parse_notelist(file);
                assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);
                count += 1;
            }
        }

        assert_eq!(count, 41, "Should have parsed 41 .nl files");
    }
}
