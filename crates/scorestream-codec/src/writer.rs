use scorestream_core::*;
use std::{collections::BTreeMap, fmt::Write};
/// Canonical text of a validated document: deterministic ordering, exact numbers,
/// quoted text, LF, one space, and a final newline. Input comments are trivia.
pub fn canonical(document: &Document) -> String {
    let mut output = String::new();
    match document.document_kind {
        DocumentKind::Score => output.push_str("!score 1.1 unit=q\n"),
        DocumentKind::Fragment => {
            writeln!(
                output,
                "!fragment 1.1 unit=q span={}",
                document.span.as_ref().unwrap_or(&document.end)
            )
            .unwrap();
        }
    }
    for part in &document.parts {
        write!(
            output,
            "!part {} instrument={} staves={}",
            part.id, part.instrument, part.staves
        )
        .unwrap();
        if let Some(name) = &part.display_name {
            // JSON permits raw DEL/C1 characters, but SFS physical records do
            // not. Escape these XML-legal label characters without changing
            // their decoded text or weakening the lexer boundary.
            let encoded = serde_json::to_string(name).expect("Unicode text");
            let encoded = encoded.chars().fold(String::new(), |mut text, c| {
                if matches!(c, '\u{7f}'..='\u{9f}') {
                    write!(text, "\\u{:04x}", c as u32).unwrap();
                } else {
                    text.push(c);
                }
                text
            });
            write!(output, " display_name={}", encoded).unwrap();
        }
        output.push('\n');
    }
    for voice in &document.voices {
        writeln!(
            output,
            "!voice {} part={} staff={}",
            voice.id, voice.part, voice.staff
        )
        .unwrap();
    }
    if !document.metadata.is_empty() {
        output.push_str("!meta");
        output.push_str(&crate::metadata::write(&document.metadata));
        output.push('\n');
    }
    for extension in &document.required_extensions {
        writeln!(output, "!requires {extension}").unwrap();
    }
    let voice_order: BTreeMap<&str, usize> = document
        .voices
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.as_str(), i))
        .collect();
    let mut frames = document.frames.iter().collect::<Vec<_>>();
    frames.sort_by(|a, b| a.time.cmp(&b.time));
    for frame in frames {
        writeln!(output, "@{}", frame.time).unwrap();
        let mut records = frame.records.iter().collect::<Vec<_>>();
        // Stable sorting is essential: simultaneous chord/grace members retain
        // their author's order instead of being sorted by pitch or identity.
        records.sort_by_key(|r| match r {
            Record::Measure(m) => (0, 0, m.target.as_str()),
            Record::State(s) => (1, 0, s.target.as_str()),
            Record::Event(e) => (
                2,
                *voice_order.get(e.voice.as_str()).unwrap_or(&usize::MAX),
                e.voice.as_str(),
            ),
            Record::Mark(_) => (3, 0, ""),
            Record::Relation(_) => (4, 0, ""),
        });
        for record in records {
            let (category, kind, attributes) = match record {
                Record::Event(e) => {
                    write!(output, "{} {}:{}", e.voice, e.pitch, e.duration).unwrap();
                    ("event", "", &e.attributes)
                }
                Record::State(s) => {
                    write!(output, "= {}", s.target).unwrap();
                    ("state", "", &s.attributes)
                }
                Record::Measure(m) => {
                    write!(output, "| {} {} len={}", m.target, m.id, m.length).unwrap();
                    ("measure", "", &m.attributes)
                }
                Record::Mark(m) => {
                    write!(output, "+ {} {}", m.target, m.kind).unwrap();
                    ("mark", m.kind.as_str(), &m.attributes)
                }
                Record::Relation(r) => {
                    write!(output, "~ {} {} {}={}", r.kind, r.id, r.phase, r.anchor).unwrap();
                    ("relation", r.kind.as_str(), &r.attributes)
                }
            };
            write_attributes(&mut output, category, kind, attributes);
            output.push('\n');
        }
        output.push_str(".\n");
    }
    writeln!(output, "!end at={}", document.end).unwrap();
    output
}
fn write_attributes(output: &mut String, category: &str, kind: &str, attributes: &Attributes) {
    // Identity first, then stable registry-independent lexical field order.
    let ordered = attributes
        .get_key_value("id")
        .into_iter()
        .chain(attributes.iter().filter(|(key, _)| key.as_str() != "id"));
    for (key, value) in ordered {
        write!(output, " {key}=").unwrap();
        match value {
            Scalar::String(s) if is_text_attribute(category, kind, key) => {
                let encoded = serde_json::to_string(s).expect("Unicode strings serialize");
                for c in encoded.chars() {
                    if matches!(c, '\u{7f}'..='\u{9f}') {
                        write!(output, "\\u{:04x}", c as u32).unwrap();
                    } else {
                        output.push(c);
                    }
                }
            }
            Scalar::String(s) => output.push_str(s),
            Scalar::Integer(i) => write!(output, "{i}").unwrap(),
            Scalar::Boolean(b) => write!(output, "{b}").unwrap(),
        }
    }
}
