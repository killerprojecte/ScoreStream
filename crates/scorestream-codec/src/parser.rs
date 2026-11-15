// Keep the short-lived, fail-fast diagnostic inline. The public codec returns
// a Vec<Diagnostic>; an extra heap allocation here would immediately be undone.
#![allow(clippy::result_large_err)]
use crate::lexer::{Token, tokenize};
use scorestream_core::*;
use std::collections::{BTreeMap, BTreeSet};
type ParseResult<T> = Result<T, Diagnostic>;

/// Parse a complete score. Fragments require `parse_with_context` instead.
/// Returns no partial document on syntax or semantic errors.
pub fn parse(source: &str) -> Result<Document, Vec<Diagnostic>> {
    parse_inner(source, None)
}
pub fn parse_with_context(
    source: &str,
    context: &FragmentContext,
) -> Result<Document, Vec<Diagnostic>> {
    parse_inner(source, Some(context))
}
fn parse_inner(
    source: &str,
    context: Option<&FragmentContext>,
) -> Result<Document, Vec<Diagnostic>> {
    let mut document = read(source).map_err(|e| vec![e])?;
    let diagnostics = validate(&document, context);
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        return Err(diagnostics);
    }
    document.diagnostics = diagnostics;
    assign_event_ids(&mut document);
    Ok(document)
}
fn read(source: &str) -> ParseResult<Document> {
    let mut document: Option<Document> = None;
    let mut frame: Option<Frame> = None;
    let mut frames_started = false;
    let mut ended = false;
    let mut last_source = SourceRange::line(1, 0);
    for (index, raw) in source.split('\n').enumerate() {
        let line_number = index + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let line = raw.trim_matches([' ', '\t']);
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let location = SourceRange::line(line_number, raw.chars().count());
        last_source = location;
        let tokens = tokenize(raw, line_number)?;
        let words = tokens.iter().map(|t| t.text).collect::<Vec<_>>();
        let head = words[0];
        if ended {
            return Err(Diagnostic::error(
                "document_trailing_record",
                "No records are allowed after !end",
                location,
            ));
        }
        if document.is_none() {
            if head != "!score" && head != "!fragment" {
                return Err(Diagnostic::error(
                    "header_required",
                    "Document must begin with !score or !fragment",
                    location,
                ));
            }
            if words.get(1) != Some(&"1.1") {
                return Err(Diagnostic::error(
                    "format_unsupported",
                    "Only ScoreStream version 1.1 is supported",
                    location,
                ));
            }
            let kind = if head == "!score" {
                DocumentKind::Score
            } else {
                DocumentKind::Fragment
            };
            let attrs = raw_attributes(&tokens[2..], location)?;
            exact_keys(
                &attrs,
                if kind == DocumentKind::Score {
                    &["unit"]
                } else {
                    &["unit", "span"]
                },
                location,
            )?;
            if attrs.get("unit").map(String::as_str) != Some("q") {
                return Err(Diagnostic::error(
                    "unit_unsupported",
                    "Time unit must be q",
                    location,
                ));
            }
            let mut doc = Document::new(kind);
            doc.source = location;
            if kind == DocumentKind::Fragment {
                doc.span = Some(positive_rational(
                    required(&attrs, "span", location)?,
                    location,
                )?);
            }
            document = Some(doc);
            continue;
        }
        let doc = document.as_mut().unwrap();
        if let Some(time) = head.strip_prefix('@') {
            if frame.is_some() {
                return Err(Diagnostic::error(
                    "frame_uncommitted",
                    "Close the previous frame with a dot before starting another",
                    location,
                ));
            }
            if words.len() != 1 {
                return Err(Diagnostic::error(
                    "frame_syntax",
                    "Frame header contains only @time",
                    location,
                ));
            }
            frame = Some(Frame {
                time: nonnegative_rational(time, location)?,
                records: Vec::new(),
                source: location,
            });
            frames_started = true;
            continue;
        }
        if head == "." {
            if words.len() != 1 {
                return Err(Diagnostic::error(
                    "frame_syntax",
                    "Frame closing dot must be alone",
                    location,
                ));
            }
            let completed = frame.take().ok_or_else(|| {
                Diagnostic::error("frame_unopened", "No frame to close", location)
            })?;
            doc.frames.push(completed);
            continue;
        }
        if head == "!end" {
            if frame.is_some() {
                return Err(Diagnostic::error(
                    "frame_uncommitted",
                    "Close the current frame with a dot before !end",
                    location,
                ));
            }
            let attrs = raw_attributes(&tokens[1..], location)?;
            exact_keys(&attrs, &["at"], location)?;
            doc.end = nonnegative_rational(required(&attrs, "at", location)?, location)?;
            ended = true;
            continue;
        }
        if head.starts_with('!') {
            if frame.is_some() || frames_started {
                return Err(Diagnostic::error(
                    "declaration_order",
                    "Declarations must precede all frames",
                    location,
                ));
            }
            if doc.document_kind == DocumentKind::Fragment {
                return Err(Diagnostic::error(
                    "fragment_global_state",
                    "Fragments cannot declare parts, voices, metadata, or extensions",
                    location,
                ));
            }
            match head {
                "!part" => {
                    let id = identifier(words.get(1).copied(), location)?;
                    let attrs = raw_attributes(&tokens[2..], location)?;
                    allowed_keys(&attrs, &["instrument", "staves", "display_name"], location)?;
                    let instrument =
                        atom(required(&attrs, "instrument", location)?, location)?.to_owned();
                    doc.parts.push(Part {
                        id: id.into(),
                        instrument,
                        staves: positive_u32(required(&attrs, "staves", location)?, location)?,
                        display_name: attrs
                            .get("display_name")
                            .map(|value| {
                                serde_json::from_str::<String>(value).map_err(|_| {
                                    Diagnostic::error(
                                        "string_invalid",
                                        "display_name requires a valid JSON-quoted string",
                                        location,
                                    )
                                })
                            })
                            .transpose()?,
                        source: location,
                    });
                }
                "!voice" => {
                    let id = identifier(words.get(1).copied(), location)?;
                    let attrs = raw_attributes(&tokens[2..], location)?;
                    exact_keys(&attrs, &["part", "staff"], location)?;
                    doc.voices.push(Voice {
                        id: id.into(),
                        part: identifier(Some(required(&attrs, "part", location)?), location)?
                            .into(),
                        staff: positive_u32(required(&attrs, "staff", location)?, location)?,
                        source: location,
                    });
                }
                "!meta" => {
                    crate::metadata::read(&mut doc.metadata, &tokens[1..], location)?;
                }
                "!requires" => {
                    if words.len() != 2 {
                        return Err(Diagnostic::error(
                            "requirement_syntax",
                            "Expected !requires namespace@version",
                            location,
                        ));
                    }
                    // Unknown mandatory extensions cannot be semantically accepted.
                    doc.required_extensions.push(words[1].into());
                }
                _ => {
                    return Err(Diagnostic::error(
                        "record_unknown",
                        format!("Unknown declaration {head}"),
                        location,
                    ));
                }
            }
            continue;
        }
        let current = frame.as_mut().ok_or_else(|| {
            Diagnostic::error(
                "record_outside_frame",
                "Musical records require an open @time frame",
                location,
            )
        })?;
        let record = match head {
            "=" => {
                let target = words.get(1).ok_or_else(|| {
                    Diagnostic::error("state_syntax", "State requires a scope", location)
                })?;
                let attrs = attributes("state", "", &tokens[2..], location)?;
                if attrs.is_empty() {
                    return Err(Diagnostic::error(
                        "field_required",
                        "State requires at least one attribute",
                        location,
                    ));
                }
                Record::State(State {
                    target: (*target).into(),
                    attributes: attrs,
                    source: location,
                })
            }
            "|" => {
                if words.len() < 4 {
                    return Err(Diagnostic::error(
                        "measure_syntax",
                        "Measure requires target, identity, and len",
                        location,
                    ));
                }
                let id = identifier(Some(words[2]), location)?;
                let mut raw = raw_attributes(&tokens[3..], location)?;
                let length = positive_rational(required(&raw, "len", location)?, location)?;
                raw.remove("len");
                Record::Measure(Measure {
                    target: words[1].into(),
                    id: id.into(),
                    length,
                    attributes: convert_attributes("measure", "", raw, location)?,
                    source: location,
                })
            }
            "+" => {
                if words.len() < 3 {
                    return Err(Diagnostic::error(
                        "mark_syntax",
                        "Mark requires target and kind",
                        location,
                    ));
                }
                Record::Mark(Mark {
                    target: words[1].into(),
                    kind: words[2].into(),
                    attributes: attributes("mark", words[2], &tokens[3..], location)?,
                    source: location,
                })
            }
            "~" => {
                if words.len() < 4 {
                    return Err(Diagnostic::error(
                        "relation_syntax",
                        "Relation requires kind, identity, and anchor phase",
                        location,
                    ));
                }
                let id = identifier(Some(words[2]), location)?;
                let mut raw = raw_attributes(&tokens[3..], location)?;
                let phases = ["start", "continue", "end"]
                    .iter()
                    .filter(|p| raw.contains_key(**p))
                    .copied()
                    .collect::<Vec<_>>();
                if phases.len() != 1 {
                    return Err(Diagnostic::error(
                        "relation_phase",
                        "Relation record needs exactly one start, continue, or end",
                        location,
                    ));
                }
                let phase = match phases[0] {
                    "start" => RelationPhase::Start,
                    "continue" => RelationPhase::Continue,
                    _ => RelationPhase::End,
                };
                let anchor = raw.remove(phases[0]).unwrap();
                if !anchor.strip_prefix('#').is_some_and(is_identifier) {
                    return Err(Diagnostic::error(
                        "reference_syntax",
                        "Anchor must be # followed by an event identity",
                        location,
                    ));
                }
                Record::Relation(Relation {
                    kind: words[1].into(),
                    id: id.into(),
                    phase,
                    anchor,
                    attributes: convert_attributes("relation", words[1], raw, location)?,
                    source: location,
                })
            }
            _ => {
                identifier(Some(head), location)?;
                let token = words.get(1).ok_or_else(|| {
                    Diagnostic::error("event_syntax", "Event requires pitch:duration", location)
                })?;
                let (pitch, duration) = token.rsplit_once(':').ok_or_else(|| {
                    Diagnostic::error(
                        "event_syntax",
                        "Event requires explicit pitch:duration",
                        location,
                    )
                })?;
                let pitch: Pitch = pitch
                    .parse()
                    .map_err(|e| Diagnostic::error("pitch_invalid", e, location))?;
                let duration: Duration = duration
                    .parse()
                    .map_err(|e: RationalError| Diagnostic::error(e.code, e.message, location))?;
                Record::Event(Event {
                    voice: head.into(),
                    pitch,
                    duration,
                    attributes: attributes("event", "", &tokens[2..], location)?,
                    source: location,
                })
            }
        };
        current.records.push(record);
    }
    if frame.is_some() {
        return Err(Diagnostic::error(
            "frame_uncommitted",
            "Incomplete draft: frame is missing its closing dot",
            last_source,
        ));
    }
    if !ended {
        return Err(Diagnostic::error(
            "document_unclosed",
            "Complete document requires !end at=<position>",
            last_source,
        ));
    }
    document.ok_or_else(|| Diagnostic::error("header_required", "Document is empty", last_source))
}
pub(crate) fn raw_attributes(
    tokens: &[Token<'_>],
    source: SourceRange,
) -> ParseResult<BTreeMap<String, String>> {
    let mut attrs = BTreeMap::new();
    for token in tokens {
        let location = SourceRange {
            column: token.column,
            end_column: token.column + token.text.chars().count(),
            ..source
        };
        let (key, value) = token.text.split_once('=').ok_or_else(|| {
            Diagnostic::error("attribute_syntax", "Expected name=value", location)
        })?;
        if !(is_identifier(key) || key.starts_with("x.") && key.split('.').all(is_identifier))
            || value.is_empty()
        {
            return Err(Diagnostic::error(
                "attribute_syntax",
                "Invalid attribute name or empty value",
                location,
            ));
        }
        if attrs.insert(key.into(), value.into()).is_some() {
            return Err(Diagnostic::error(
                "attribute_duplicate",
                format!("Repeated attribute {key}"),
                location,
            ));
        }
    }
    Ok(attrs)
}
fn attributes(
    category: &str,
    kind: &str,
    tokens: &[Token<'_>],
    source: SourceRange,
) -> ParseResult<Attributes> {
    convert_attributes(category, kind, raw_attributes(tokens, source)?, source)
}
fn convert_attributes(
    category: &str,
    kind: &str,
    raw: BTreeMap<String, String>,
    source: SourceRange,
) -> ParseResult<Attributes> {
    raw.into_iter()
        .map(|(key, value)| {
            let Some(ty) = attribute_type(category, kind, &key) else {
                return Err(Diagnostic::error(
                    if key.starts_with("x.") {
                        "extension_unsupported"
                    } else {
                        "field_unknown"
                    },
                    format!("Unknown {category} field {key}"),
                    source,
                ));
            };
            let scalar = if ty == "string" {
                Scalar::String(serde_json::from_str::<String>(&value).map_err(|_| {
                    Diagnostic::error(
                        "string_invalid",
                        format!("{key} requires a valid JSON-quoted string"),
                        source,
                    )
                })?)
            } else {
                atom(&value, source)?;
                match ty {
                    "boolean" => Scalar::Boolean(match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => {
                            return Err(Diagnostic::error(
                                "field_value",
                                format!("{key} must be true or false"),
                                source,
                            ));
                        }
                    }),
                    "positive_integer" | "integer" | "nonnegative_integer" | "nonzero_integer" => {
                        Scalar::Integer(integer(&value, source)?)
                    }
                    "key" if value != "unset" => Scalar::Integer(integer(&value, source)?),
                    _ => Scalar::String(value),
                }
            };
            Ok((key, scalar))
        })
        .collect()
}
fn identifier(value: Option<&str>, source: SourceRange) -> ParseResult<&str> {
    value.filter(|s| is_identifier(s)).ok_or_else(|| {
        Diagnostic::error("identifier_invalid", "Expected an ASCII identifier", source)
    })
}
fn atom(value: &str, source: SourceRange) -> ParseResult<&str> {
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"')
    {
        Err(Diagnostic::error(
            "atom_invalid",
            "Expected an unquoted atom",
            source,
        ))
    } else {
        Ok(value)
    }
}
fn integer(value: &str, source: SourceRange) -> ParseResult<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || value == "-0"
    {
        return Err(Diagnostic::error(
            "integer_lexical",
            "Expected canonical integer",
            source,
        ));
    }
    value.parse().map_err(|_| Diagnostic::error("integer_range_unsupported","This notation integer exceeds the supported signed 64-bit range; musical time remains arbitrary precision",source))
}
fn positive_u32(value: &str, source: SourceRange) -> ParseResult<u32> {
    let n = integer(value, source)?;
    u32::try_from(n).ok().filter(|n| *n > 0).ok_or_else(|| {
        Diagnostic::error(
            "staff_range",
            "Staff count/index must be a positive 32-bit integer",
            source,
        )
    })
}
fn positive_rational(value: &str, source: SourceRange) -> ParseResult<Rational> {
    let r: Rational = value
        .parse()
        .map_err(|e: RationalError| Diagnostic::error(e.code, e.message, source))?;
    if !r.is_positive() {
        Err(Diagnostic::error(
            "duration_not_positive",
            "Expected positive duration",
            source,
        ))
    } else {
        Ok(r)
    }
}
fn nonnegative_rational(value: &str, source: SourceRange) -> ParseResult<Rational> {
    let r: Rational = value
        .parse()
        .map_err(|e: RationalError| Diagnostic::error(e.code, e.message, source))?;
    if r.is_negative() {
        Err(Diagnostic::error(
            "time_negative",
            "Position must be nonnegative",
            source,
        ))
    } else {
        Ok(r)
    }
}
fn required<'a>(
    attrs: &'a BTreeMap<String, String>,
    key: &str,
    source: SourceRange,
) -> ParseResult<&'a str> {
    attrs.get(key).map(String::as_str).ok_or_else(|| {
        Diagnostic::error(
            "field_required",
            format!("Missing required field {key}"),
            source,
        )
    })
}
fn exact_keys(
    attrs: &BTreeMap<String, String>,
    allowed: &[&str],
    source: SourceRange,
) -> ParseResult<()> {
    for key in allowed {
        required(attrs, key, source)?;
    }
    allowed_keys(attrs, allowed, source)
}
fn allowed_keys(
    attrs: &BTreeMap<String, String>,
    allowed: &[&str],
    source: SourceRange,
) -> ParseResult<()> {
    if let Some(key) = attrs.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(Diagnostic::error(
            "field_unknown",
            format!("Unexpected field {key}"),
            source,
        ));
    }
    Ok(())
}
fn assign_event_ids(doc: &mut Document) {
    let mut used: BTreeSet<String> = doc
        .events()
        .filter_map(|(_, e)| e.id().map(str::to_owned))
        .collect();
    let mut next = 1u64;
    for frame in &mut doc.frames {
        for record in &mut frame.records {
            if let Record::Event(e) = record {
                if e.id().is_none() {
                    loop {
                        let id = format!("n{next}");
                        next += 1;
                        if used.insert(id.clone()) {
                            e.attributes.insert("id".into(), Scalar::String(id));
                            break;
                        }
                    }
                }
            }
        }
    }
}
