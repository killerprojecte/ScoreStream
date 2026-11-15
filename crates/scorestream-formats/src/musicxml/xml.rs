//! Incremental, UTF-8 XML reader. Retains only a header or one measure, never a score DOM.
//! External DOCTYPE metadata is inert; no resolver or DTD/entity expansion exists here.
use crate::{Format, MappingEffect, MappingReport};
use quick_xml::{Reader, events::Event};
use scorestream_core::SourceRange;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Read},
};

const MAX_DEPTH: usize = 48;
const MAX_TOKEN_BYTES: usize = 1024 * 1024;
const MAX_FRAGMENT_NODES: usize = 100_000;
const MAX_FRAGMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_NAMES: usize = 256;
const MAX_NAME_BYTES: usize = 256;

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
    pub text: String,
    pub source: SourceRange,
    pub path: String,
    child_counts: BTreeMap<String, usize>,
    retained_nodes: usize,
}
impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |n| n.name == name)
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.child(name).map(|n| n.text.trim())
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }
    pub fn error(
        &self,
        report: &mut MappingReport,
        effect: MappingEffect,
        feature: &str,
        message: impl Into<String>,
    ) {
        report.push_source(
            Format::MusicXml,
            effect,
            feature,
            &self.path,
            message,
            Some(self.source),
        );
    }
}
pub(super) fn fail(
    feature: &str,
    message: impl Into<String>,
    effect: MappingEffect,
) -> MappingReport {
    let mut report = MappingReport::default();
    report.push(Format::MusicXml, effect, feature, "/", message);
    report
}

/// Well-formedness and resource accounting, not a claim of notation support.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ScanSummary {
    pub bytes: u64,
    pub elements: u64,
    pub parts: u64,
    pub measures: u64,
    pub notes: u64,
    pub max_retained_nodes: usize,
    #[serde(skip)]
    pub(crate) fingerprint: [u8; 32],
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    PartStart,
    Complete,
}

/// A token-local budget bounds quick-xml's allocation *before* a token is accumulated.
/// This is not a document-size budget. Line positions are counted as input is consumed.
struct Input<R> {
    inner: R,
    line: usize,
    column: usize,
    token_bytes: usize,
    consumed: u64,
    previous: (usize, usize),
    fingerprint: Sha256,
}
impl<R: BufRead> Read for Input<R> {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        let count = {
            let available = self.fill_buf()?;
            let count = available.len().min(target.len());
            target[..count].copy_from_slice(&available[..count]);
            count
        };
        self.consume(count);
        Ok(count)
    }
}
impl<R: BufRead> BufRead for Input<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let available = self.inner.fill_buf()?;
        if self.token_bytes >= MAX_TOKEN_BYTES && !available.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "xml.token.limit",
            ));
        }
        Ok(&available[..available.len().min(MAX_TOKEN_BYTES - self.token_bytes)])
    }
    fn consume(&mut self, count: usize) {
        // quick-xml consumes only bytes returned by fill_buf; no read-ahead is retained here.
        let bytes = self.inner.fill_buf().expect("buffer was just filled");
        self.fingerprint.update(&bytes[..count]);
        for byte in &bytes[..count] {
            self.previous = (self.line, self.column);
            self.consumed += 1;
            if *byte == b'\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        self.token_bytes += count;
        self.inner.consume(count);
    }
}

/// Inspect large files without retaining the document or requiring semantic mapping.
pub fn scan<R: BufRead>(input: R) -> Result<ScanSummary, MappingReport> {
    walk(input, |_, _, _| Ok(()))
}

/// Emit a complete header, measure, stripped part, and finally stripped root.
/// child_counts retain cardinality after emitted children have been released.
pub(super) fn walk<R: BufRead>(
    input: R,
    mut visit: impl FnMut(&Node, Option<&Node>, Phase) -> Result<(), MappingReport>,
) -> Result<ScanSummary, MappingReport> {
    let mut reader = Reader::from_reader(Input {
        inner: input,
        line: 1,
        column: 1,
        token_bytes: 0,
        consumed: 0,
        previous: (1, 1),
        fingerprint: Sha256::new(),
    });
    reader.config_mut().check_comments = true;
    let mut buffer = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut seen_root = false;
    let mut closed_root = false;
    let mut seen_declaration = false;
    let mut seen_doctype = false;
    let mut summary = ScanSummary::default();
    let mut retained_nodes = 0usize;
    let mut fragment_bytes = 0usize;
    loop {
        // quick-xml may consume the next '<' before emitting preceding text.
        // buffer_position excludes this look-ahead byte, so use its preceding position.
        let input = reader.get_ref();
        let (line, column) = if reader.buffer_position() < input.consumed {
            input.previous
        } else {
            (input.line, input.column)
        };
        let source = SourceRange {
            line,
            column,
            end_line: line,
            end_column: column,
        };
        reader.get_mut().token_bytes = 0;
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).map_err(|e| {
            if reader.get_ref().token_bytes >= MAX_TOKEN_BYTES {
                fail(
                    "xml.token",
                    "An XML token exceeds the 1 MiB structural safety budget",
                    MappingEffect::ResourceLimit,
                )
            } else {
                fail("xml.syntax", e.to_string(), MappingEffect::Invalid)
            }
        })?;
        let raw = std::str::from_utf8(event.as_ref())
            .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
        if raw.chars().any(|c| !valid_char(c)) {
            return Err(fail(
                "xml.character",
                "Character forbidden by XML 1.0",
                MappingEffect::Invalid,
            ));
        }
        // Only a retained fragment is bounded. Repeated measures and parts reset this budget.
        if matches!(event, Event::Start(_) | Event::Empty(_)) {
            fragment_bytes = fragment_bytes.saturating_add(raw.len());
        }
        let completed = match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if stack.len() >= MAX_DEPTH || retained_nodes >= MAX_FRAGMENT_NODES {
                    return Err(fail(
                        "xml.structure",
                        "XML exceeds depth 48 or 100000 retained elements in one header/measure",
                        MappingEffect::ResourceLimit,
                    ));
                }
                if stack.is_empty() {
                    if seen_root {
                        return Err(fail(
                            "xml.root",
                            "Multiple root elements",
                            MappingEffect::Invalid,
                        ));
                    }
                    seen_root = true;
                }
                let name = std::str::from_utf8(e.name().as_ref())
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?
                    .to_owned();
                if !xml_name(&name) {
                    return Err(fail(
                        "xml.name",
                        "Invalid XML element name",
                        MappingEffect::Invalid,
                    ));
                }
                if name.len() > MAX_NAME_BYTES {
                    return Err(fail(
                        "xml.name",
                        "XML name exceeds 256 bytes",
                        MappingEffect::ResourceLimit,
                    ));
                }
                let mut attrs = BTreeMap::new();
                for attr in e.attributes() {
                    if attrs.len() >= MAX_NAMES {
                        return Err(fail(
                            "xml.attribute",
                            "An element has more than 256 attributes",
                            MappingEffect::ResourceLimit,
                        ));
                    }
                    let attr = attr.map_err(|e| {
                        fail("xml.attribute", e.to_string(), MappingEffect::Invalid)
                    })?;
                    let name = std::str::from_utf8(attr.key.as_ref())
                        .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?
                        .to_owned();
                    if !xml_name(&name) {
                        return Err(fail(
                            "xml.name",
                            "Invalid XML attribute name",
                            MappingEffect::Invalid,
                        ));
                    }
                    if attr.value.contains(&b'<') {
                        return Err(fail(
                            "xml.attribute",
                            "Unescaped less-than sign in attribute",
                            MappingEffect::Invalid,
                        ));
                    }
                    if name.len() > MAX_NAME_BYTES {
                        return Err(fail(
                            "xml.name",
                            "XML attribute name exceeds 256 bytes",
                            MappingEffect::ResourceLimit,
                        ));
                    }
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| fail("xml.entity", e.to_string(), MappingEffect::Invalid))?
                        .into_owned();
                    if value.chars().any(|c| !valid_char(c)) {
                        return Err(fail(
                            "xml.character",
                            "Character forbidden by XML 1.0",
                            MappingEffect::Invalid,
                        ));
                    }
                    attrs.insert(name, value);
                }
                let index = if let Some(parent) = stack.last_mut() {
                    if !parent.child_counts.contains_key(&name)
                        && parent.child_counts.len() >= MAX_NAMES
                    {
                        return Err(fail(
                            "xml.structure",
                            "An element has more than 256 distinct child names",
                            MappingEffect::ResourceLimit,
                        ));
                    }
                    let count = parent.child_counts.entry(name.clone()).or_default();
                    *count += 1;
                    *count
                } else {
                    1
                };
                let path = format!(
                    "{}/{}[{index}]",
                    stack.last().map(|n| n.path.as_str()).unwrap_or(""),
                    name
                );
                // Paths are copied for precise diagnostics; charge that retained memory too.
                fragment_bytes = fragment_bytes.saturating_add(path.len());
                let node = Node {
                    name,
                    attrs,
                    children: vec![],
                    text: String::new(),
                    source,
                    path,
                    child_counts: BTreeMap::new(),
                    retained_nodes: 1,
                };
                summary.elements += 1;
                if stack.len() == 1 && node.name == "part" {
                    summary.parts += 1;
                }
                if stack.len() == 2 && node.name == "measure" {
                    summary.measures += 1;
                }
                if stack.len() == 3 && node.name == "note" {
                    summary.notes += 1;
                }
                retained_nodes += 1;
                summary.max_retained_nodes = summary.max_retained_nodes.max(retained_nodes);
                if stack.len() == 1 && node.name == "part" {
                    visit(&node, stack.last(), Phase::PartStart)?;
                }
                if matches!(event, Event::Empty(_)) {
                    Some(node)
                } else {
                    stack.push(node);
                    None
                }
            }
            Event::End(_) => Some(stack.pop().ok_or_else(|| {
                fail(
                    "xml.syntax",
                    "Unexpected closing tag",
                    MappingEffect::Invalid,
                )
            })?),
            Event::Text(e) => {
                if e.as_ref().windows(3).any(|v| v == b"]]>") {
                    return Err(fail(
                        "xml.syntax",
                        "CDATA end marker outside CDATA",
                        MappingEffect::Invalid,
                    ));
                }
                let value = e
                    .xml10_content()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                fragment_bytes += append_text(&mut stack, &value, false)?;
                None
            }
            Event::CData(e) => {
                let value = e
                    .xml10_content()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                fragment_bytes += append_text(&mut stack, &value, true)?;
                None
            }
            Event::GeneralRef(e) => {
                let name = e
                    .decode()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                let escaped = format!("&{name};");
                let value = quick_xml::escape::unescape(&escaped)
                    .map_err(|e| fail("xml.entity", e.to_string(), MappingEffect::Invalid))?;
                fragment_bytes += append_text(&mut stack, &value, true)?;
                None
            }
            Event::DocType(e) => {
                if seen_root || seen_doctype {
                    return Err(fail(
                        "xml.doctype",
                        "Misplaced or repeated DOCTYPE",
                        MappingEffect::Invalid,
                    ));
                }
                let text = std::str::from_utf8(e.as_ref())
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                if !inert_doctype(text) {
                    return Err(fail(
                        "xml.doctype",
                        "Only the inert official MusicXML 4.0 partwise PUBLIC declaration is accepted; DTD subsets and entity declarations are forbidden",
                        MappingEffect::Unsupported,
                    ));
                }
                seen_doctype = true;
                None
            }
            Event::PI(_) => {
                return Err(fail(
                    "xml.processing_instruction",
                    "Processing instructions are not mapped",
                    MappingEffect::Unsupported,
                ));
            }
            Event::Decl(e) => {
                if seen_declaration || seen_root || seen_doctype {
                    return Err(fail(
                        "xml.declaration",
                        "Misplaced or repeated declaration",
                        MappingEffect::Invalid,
                    ));
                }
                seen_declaration = true;
                if e.version()
                    .map_err(|e| fail("xml.declaration", e.to_string(), MappingEffect::Invalid))?
                    .as_ref()
                    != b"1.0"
                {
                    return Err(fail(
                        "xml.version",
                        "Only XML 1.0 is supported",
                        MappingEffect::Unsupported,
                    ));
                }
                if e.encoding()
                    .transpose()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?
                    .is_some_and(|v| !v.eq_ignore_ascii_case(b"utf-8"))
                {
                    return Err(fail(
                        "xml.encoding",
                        "Only UTF-8 XML is supported",
                        MappingEffect::Unsupported,
                    ));
                }
                None
            }
            Event::Comment(_) => None,
            Event::Eof => break,
        };
        if fragment_bytes > MAX_FRAGMENT_BYTES {
            return Err(fail(
                "xml.fragment",
                "One XML header or measure exceeds the 8 MiB content/path safety budget",
                MappingEffect::ResourceLimit,
            ));
        }
        if let Some(node) = completed {
            let emit = stack.len() <= 1 || (stack.len() == 2 && stack[1].name == "part");
            if emit {
                visit(&node, stack.last(), Phase::Complete)?;
                retained_nodes -= node.retained_nodes;
                fragment_bytes = 0;
                if stack.is_empty() {
                    closed_root = true;
                }
            } else if let Some(parent) = stack.last_mut() {
                parent.retained_nodes += node.retained_nodes;
                parent.children.push(node);
            }
        }
        // Whitespace and comments between units do not accumulate document-sized text.
        if stack.is_empty() {
            fragment_bytes = 0;
        }
    }
    if !stack.is_empty() {
        return Err(fail(
            "xml.syntax",
            "Unclosed XML elements",
            MappingEffect::Invalid,
        ));
    }
    if !closed_root {
        return Err(fail("xml.root", "Missing root", MappingEffect::Invalid));
    }
    summary.bytes = reader.buffer_position();
    summary.fingerprint = reader.get_ref().fingerprint.clone().finalize().into();
    Ok(summary)
}
fn append_text(
    stack: &mut [Node],
    value: &str,
    require_parent: bool,
) -> Result<usize, MappingReport> {
    if value.chars().any(|c| !valid_char(c)) {
        return Err(fail(
            "xml.character",
            "Character forbidden by XML 1.0",
            MappingEffect::Invalid,
        ));
    }
    if let Some(parent) = stack.last_mut() {
        // Container whitespace is insignificant; retaining it would grow with measure count.
        if stack_container(&parent.name) && value.trim().is_empty() {
            return Ok(0);
        }
        if parent.text.len().saturating_add(value.len()) > MAX_TOKEN_BYTES {
            return Err(fail(
                "xml.text",
                "One element's text exceeds 1 MiB",
                MappingEffect::ResourceLimit,
            ));
        }
        parent.text.push_str(value);
        return Ok(value.len());
    } else if require_parent || !value.trim().is_empty() {
        return Err(fail(
            "xml.syntax",
            "Text or entity outside root",
            MappingEffect::Invalid,
        ));
    }
    Ok(0)
}
fn stack_container(name: &str) -> bool {
    matches!(
        name,
        "score-partwise"
            | "part"
            | "measure"
            | "work"
            | "identification"
            | "part-list"
            | "score-part"
            | "score-instrument"
            | "attributes"
            | "key"
            | "time"
            | "clef"
            | "transpose"
            | "note"
            | "pitch"
            | "time-modification"
            | "backup"
            | "forward"
            | "rest"
            | "chord"
            | "dot"
            | "normal-dot"
    )
}

fn xml_name(name: &str) -> bool {
    fn initial(c: char) -> bool {
        matches!(c, ':' | '_' | 'A'..='Z' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
    }
    let mut chars = name.chars();
    chars.next().is_some_and(initial) && chars.all(|c| initial(c) || matches!(c, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
}

fn inert_doctype(value: &str) -> bool {
    // Parse only this tiny, fixed declaration grammar; never parse a DTD or use its URI.
    let mut value = value.trim();
    for word in ["score-partwise", "PUBLIC"] {
        let Some(rest) = value.strip_prefix(word) else {
            return false;
        };
        if !rest.starts_with([' ', '\t', '\r', '\n']) {
            return false;
        }
        value = rest.trim_start();
    }
    for expected in [
        "-//Recordare//DTD MusicXML 4.0 Partwise//EN",
        "http://www.musicxml.org/dtds/partwise.dtd",
    ] {
        let Some(quote) = value.chars().next().filter(|q| matches!(*q, '\'' | '"')) else {
            return false;
        };
        value = &value[1..];
        let Some(rest) = value
            .strip_prefix(expected)
            .and_then(|v| v.strip_prefix(quote))
        else {
            return false;
        };
        if expected.starts_with("-//") && !rest.starts_with([' ', '\t', '\r', '\n']) {
            return false;
        }
        value = rest.trim_start();
    }
    value.is_empty()
}

/// Deliberately closed conversion profile, not a full MusicXML XSD validator.
pub(super) fn schema(n: &Node, report: &mut MappingReport) {
    let (attrs, children, multiple, required, text): (&[&str], &[&str], &[&str], &[&str], bool) =
        match n.name.as_str() {
            "score-partwise" => (
                &["version"],
                &[
                    "work",
                    "movement-title",
                    "identification",
                    "part-list",
                    "part",
                ],
                &["part"],
                &["part-list", "part"],
                false,
            ),
            "work" => (&[], &["work-title", "work-number"], &[], &[], false),
            "identification" => (&[], &["creator", "rights"], &["creator"], &[], false),
            "creator" => (&["type"], &[], &[], &[], true),
            "part-list" => (
                &[],
                &["score-part"],
                &["score-part"],
                &["score-part"],
                false,
            ),
            "score-part" => (
                &["id"],
                &["part-name", "score-instrument"],
                &[],
                &["part-name"],
                false,
            ),
            "score-instrument" => (
                &["id"],
                &["instrument-name"],
                &[],
                &["instrument-name"],
                false,
            ),
            "part" => (&["id"], &["measure"], &["measure"], &["measure"], false),
            "measure" => (
                &["number", "implicit", "id"],
                &[
                    "attributes",
                    "note",
                    "backup",
                    "forward",
                    "direction",
                    "sound",
                    "barline",
                ],
                &[
                    "attributes",
                    "note",
                    "backup",
                    "forward",
                    "direction",
                    "sound",
                    "barline",
                ],
                &[],
                false,
            ),
            "direction" => (
                &[],
                &["direction-type", "offset", "staff", "sound"],
                &[],
                &["direction-type"],
                false,
            ),
            "direction-type" => (&[], &["metronome"], &[], &["metronome"], false),
            "metronome" => (
                &[],
                &["beat-unit", "beat-unit-dot", "per-minute"],
                &["beat-unit-dot"],
                &["beat-unit", "per-minute"],
                false,
            ),
            "sound" => (&["tempo"], &["offset"], &[], &[], false),
            "offset" => (&["sound"], &[], &[], &[], true),
            "barline" => (
                &["id", "location"],
                &["bar-style", "ending", "repeat"],
                &[],
                &[],
                false,
            ),
            "bar-style" => (&[], &[], &[], &[], true),
            "repeat" => (&["direction", "times"], &[], &[], &[], false),
            "ending" => (&["number", "type", "default-y"], &[], &[], &[], true),
            "attributes" => (
                &[],
                &["divisions", "key", "time", "staves", "clef", "transpose"],
                &["key", "time", "clef", "transpose"],
                &[],
                false,
            ),
            "key" => (&["number"], &["fifths"], &[], &["fifths"], false),
            "time" => (
                &["number"],
                &["beats", "beat-type"],
                &[],
                &["beats", "beat-type"],
                false,
            ),
            "clef" => (
                &["number"],
                &["sign", "line"],
                &[],
                &["sign", "line"],
                false,
            ),
            "transpose" => (
                &["number"],
                &["diatonic", "chromatic", "octave-change"],
                &[],
                &["chromatic"],
                false,
            ),
            "note" => (
                &["id"],
                &[
                    "chord",
                    "pitch",
                    "rest",
                    "duration",
                    "tie",
                    "notations",
                    "voice",
                    "type",
                    "dot",
                    "time-modification",
                    "staff",
                ],
                &["dot", "tie", "notations"],
                &["duration"],
                false,
            ),
            "tie" => (&["type", "time-only"], &[], &[], &[], false),
            "notations" => (&[], &["tied"], &["tied"], &[], false),
            "tied" => (&["type", "number"], &[], &[], &[], false),
            "pitch" => (
                &[],
                &["step", "alter", "octave"],
                &[],
                &["step", "octave"],
                false,
            ),
            "time-modification" => (
                &[],
                &["actual-notes", "normal-notes", "normal-type", "normal-dot"],
                &["normal-dot"],
                &["actual-notes", "normal-notes"],
                false,
            ),
            "backup" => (&[], &["duration"], &[], &["duration"], false),
            "forward" => (
                &[],
                &["duration", "voice", "staff"],
                &[],
                &["duration"],
                false,
            ),
            "rest" | "chord" | "dot" | "normal-dot" | "beat-unit-dot" => {
                (&[], &[], &[], &[], false)
            }
            "work-title" | "work-number" | "movement-title" | "rights" | "part-name"
            | "instrument-name" | "divisions" | "staves" | "fifths" | "beats" | "beat-type"
            | "sign" | "line" | "step" | "alter" | "octave" | "duration" | "voice" | "type"
            | "staff" | "actual-notes" | "normal-notes" | "normal-type" | "diatonic"
            | "chromatic" | "octave-change" | "beat-unit" | "per-minute" => {
                (&[], &[], &[], &[], true)
            }
            _ => {
                n.error(
                    report,
                    MappingEffect::Unsupported,
                    &format!("element.{}", n.name),
                    "Element has no implemented typed mapping",
                );
                return;
            }
        };
    for key in n.attrs.keys() {
        if !attrs.contains(&key.as_str()) {
            n.error(
                report,
                MappingEffect::Unsupported,
                &format!("attribute.{key}"),
                "Attribute has no implemented typed mapping",
            );
        }
    }
    if !text && !n.text.trim().is_empty() {
        n.error(
            report,
            MappingEffect::Invalid,
            "xml.content",
            "Unexpected text content",
        );
    }
    for child in &n.children {
        if !children.contains(&child.name.as_str()) {
            child.error(
                report,
                MappingEffect::Unsupported,
                &format!("element.{}", child.name),
                "Element has no implemented mapping at this path",
            );
        } else {
            schema(child, report);
        }
    }
    for name in children {
        if !multiple.contains(name) && n.child_counts.get(*name).copied().unwrap_or(0) > 1 {
            let effect = if matches!(
                (n.name.as_str(), *name),
                ("score-part", "score-instrument")
                    | ("identification", "rights")
                    | ("time", "beats" | "beat-type")
            ) {
                MappingEffect::Unsupported
            } else {
                MappingEffect::Invalid
            };
            n.error(
                report,
                effect,
                "xml.cardinality",
                format!("Repeated {name} is outside the conversion profile"),
            );
        }
    }
    for name in required {
        if !n.child_counts.contains_key(*name) {
            n.error(
                report,
                MappingEffect::Invalid,
                "xml.cardinality",
                format!("Required {name} missing"),
            );
        }
    }
}
pub fn valid_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
pub fn escape(value: &str) -> String {
    quick_xml::escape::escape(value)
        .replace('\r', "&#13;")
        .replace('\n', "&#10;")
        .replace('\t', "&#9;")
}
