//! Bounded XML tree built with the established quick-xml parser. No DTD/entity I/O.
use crate::{Format, MappingEffect, MappingReport};
use quick_xml::{Reader, events::Event};
use scorestream_core::SourceRange;
use std::collections::BTreeMap;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 48;
#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
    pub text: String,
    pub source: SourceRange,
    pub path: String,
    child_counts: BTreeMap<String, usize>,
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
        report.push(Format::MusicXml, effect, feature, &self.path, message);
        report.diagnostics.last_mut().unwrap().source = Some(self.source);
    }
}
fn fail(feature: &str, message: impl Into<String>, effect: MappingEffect) -> MappingReport {
    let mut report = MappingReport::default();
    report.push(Format::MusicXml, effect, feature, "/", message);
    report
}
pub fn parse(source: &str) -> Result<Node, MappingReport> {
    if source.len() > MAX_BYTES {
        return Err(fail(
            "xml.size",
            "XML input exceeds 8 MiB",
            MappingEffect::ResourceLimit,
        ));
    }
    if source.chars().any(|c| !valid_char(c)) {
        return Err(fail(
            "xml.character",
            "Character forbidden by XML 1.0",
            MappingEffect::Invalid,
        ));
    }
    let mut reader = Reader::from_str(source);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0;
    let mut seen_declaration = false;
    let mut lines = vec![0usize];
    lines.extend(source.match_indices('\n').map(|(i, _)| i + 1));
    loop {
        let offset = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|e| fail("xml.syntax", e.to_string(), MappingEffect::Invalid))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                count += 1;
                if count > MAX_NODES || stack.len() >= MAX_DEPTH {
                    return Err(fail(
                        "xml.structure",
                        "XML exceeds 100000 elements or depth 48",
                        MappingEffect::ResourceLimit,
                    ));
                }
                if stack.is_empty() && root.is_some() {
                    return Err(fail(
                        "xml.root",
                        "Multiple root elements",
                        MappingEffect::Invalid,
                    ));
                }
                let name = std::str::from_utf8(e.name().as_ref())
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?
                    .to_owned();
                let mut attrs = BTreeMap::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| {
                        fail("xml.attribute", e.to_string(), MappingEffect::Invalid)
                    })?;
                    let name = std::str::from_utf8(attr.key.as_ref())
                        .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?
                        .to_owned();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| fail("xml.entity", e.to_string(), MappingEffect::Invalid))?
                        .into_owned();
                    attrs.insert(name, value);
                }
                let line = lines.partition_point(|n| *n <= offset).max(1);
                let index = stack
                    .last_mut()
                    .map(|p| {
                        let count = p.child_counts.entry(name.clone()).or_default();
                        *count += 1;
                        *count
                    })
                    .unwrap_or(1);
                let path = format!(
                    "{}/{}[{index}]",
                    stack.last().map(|p| p.path.as_str()).unwrap_or(""),
                    name
                );
                let node = Node {
                    name,
                    attrs,
                    child_counts: BTreeMap::new(),
                    children: vec![],
                    text: String::new(),
                    path,
                    source: SourceRange {
                        line,
                        column: offset - lines[line - 1] + 1,
                        end_line: line,
                        end_column: offset - lines[line - 1] + 1,
                    },
                };
                if matches!(event, Event::Empty(_)) {
                    if let Some(p) = stack.last_mut() {
                        p.children.push(node);
                    } else {
                        root = Some(node);
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or_else(|| {
                    fail(
                        "xml.syntax",
                        "Unexpected closing tag",
                        MappingEffect::Invalid,
                    )
                })?;
                if let Some(p) = stack.last_mut() {
                    p.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Event::Text(e) => {
                let value = e
                    .xml_content()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                if let Some(p) = stack.last_mut() {
                    p.text.push_str(&value);
                } else if !value.trim().is_empty() {
                    return Err(fail(
                        "xml.syntax",
                        "Text outside root",
                        MappingEffect::Invalid,
                    ));
                }
            }
            Event::CData(e) => {
                let value = e
                    .xml_content()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                if let Some(p) = stack.last_mut() {
                    p.text.push_str(&value);
                } else {
                    return Err(fail(
                        "xml.syntax",
                        "CDATA outside root",
                        MappingEffect::Invalid,
                    ));
                }
            }
            Event::GeneralRef(e) => {
                let name = e
                    .decode()
                    .map_err(|e| fail("xml.encoding", e.to_string(), MappingEffect::Invalid))?;
                let value = quick_xml::escape::unescape(&format!("&{name};"))
                    .map_err(|e| fail("xml.entity", e.to_string(), MappingEffect::Invalid))?
                    .into_owned();
                if let Some(p) = stack.last_mut() {
                    p.text.push_str(&value);
                } else {
                    return Err(fail(
                        "xml.syntax",
                        "Entity outside root",
                        MappingEffect::Invalid,
                    ));
                }
            }
            Event::DocType(_) => {
                return Err(fail(
                    "xml.doctype",
                    "DTD declarations are not accepted; no external entities are fetched",
                    MappingEffect::Unsupported,
                ));
            }
            Event::PI(_) => {
                return Err(fail(
                    "xml.processing_instruction",
                    "Processing instructions are not mapped",
                    MappingEffect::Unsupported,
                ));
            }
            Event::Decl(e) => {
                if seen_declaration || root.is_some() || !stack.is_empty() {
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
            }
            Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err(fail(
            "xml.syntax",
            "Unclosed XML elements",
            MappingEffect::Invalid,
        ));
    }
    let root = root.ok_or_else(|| fail("xml.root", "Missing root", MappingEffect::Invalid))?;
    let mut report = MappingReport::default();
    schema(&root, &mut report);
    if report.has_errors() {
        Err(report)
    } else {
        Ok(root)
    }
}
/// Deliberately closed conversion profile, not a full MusicXML XSD validator.
fn schema(n: &Node, report: &mut MappingReport) {
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
                &["attributes", "note", "backup", "forward"],
                &["attributes", "note", "backup", "forward"],
                &[],
                false,
            ),
            "attributes" => (
                &[],
                &["divisions", "key", "time", "staves", "clef"],
                &["key", "time", "clef"],
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
            "note" => (
                &["id"],
                &[
                    "chord",
                    "pitch",
                    "rest",
                    "duration",
                    "voice",
                    "type",
                    "dot",
                    "time-modification",
                    "staff",
                ],
                &["dot"],
                &["duration"],
                false,
            ),
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
            "rest" | "chord" | "dot" | "normal-dot" => (&[], &[], &[], &[], false),
            "work-title" | "work-number" | "movement-title" | "rights" | "part-name"
            | "instrument-name" | "divisions" | "staves" | "fifths" | "beats" | "beat-type"
            | "sign" | "line" | "step" | "alter" | "octave" | "duration" | "voice" | "type"
            | "staff" | "actual-notes" | "normal-notes" | "normal-type" => {
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
        if !multiple.contains(name) && n.children_named(name).count() > 1 {
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
        if n.child(name).is_none() {
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
