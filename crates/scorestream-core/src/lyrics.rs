//! Authored note lyrics, not analytical annotations or inferred performance.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.lyrics@1";
pub const KIND: &str = "scorestream.lyric";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventLyrics {
    pub event: String,
    pub lyrics: Vec<Lyric>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lyric {
    pub number: Option<String>,
    pub syllabic: Syllabic,
    pub text: String,
    pub color: Option<String>,
    pub default_x: Option<Rational>,
    pub default_y: Option<Rational>,
    pub relative_x: Option<Rational>,
    pub relative_y: Option<Rational>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Syllabic {
    Single,
    Begin,
    Middle,
    End,
}
impl Syllabic {
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Begin => "begin",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "single" => Self::Single,
            "begin" => Self::Begin,
            "middle" => Self::Middle,
            "end" => Self::End,
            _ => return None,
        })
    }
}
fn text_valid(s: &str) -> bool {
    s.chars().all(|c|matches!(c,'\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}'))
}
// XML 1.0 NameChar; MusicXML verse identifiers are NMTOKEN, not numeric indices.
pub fn verse_valid(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c|matches!(c, ':'|'_'|'A'..='Z'|'a'..='z'|'\u{c0}'..='\u{d6}'|'\u{d8}'..='\u{f6}'|'\u{f8}'..='\u{2ff}'|'\u{370}'..='\u{37d}'|'\u{37f}'..='\u{1fff}'|'\u{200c}'..='\u{200d}'|'\u{2070}'..='\u{218f}'|'\u{2c00}'..='\u{2fef}'|'\u{3001}'..='\u{d7ff}'|'\u{f900}'..='\u{fdcf}'|'\u{fdf0}'..='\u{fffd}'|'\u{10000}'..='\u{effff}'|'-'|'.'|'0'..='9'|'\u{b7}'|'\u{300}'..='\u{36f}'|'\u{203f}'..='\u{2040}'))
}
impl Lyric {
    pub fn valid(&self) -> bool {
        text_valid(&self.text)
            && self.number.as_ref().is_none_or(|s| verse_valid(s))
            && self.color.as_ref().is_none_or(|s| {
                matches!(s.len(), 7 | 9)
                    && s.starts_with('#')
                    && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
            && [
                &self.default_x,
                &self.default_y,
                &self.relative_x,
                &self.relative_y,
            ]
            .into_iter()
            .flatten()
            .all(crate::geometry::decimal)
    }
}
pub fn is_kind(s: &str) -> bool {
    s == KIND
        || s.starts_with("scorestream.lyric.")
        || s == "scorestream.lyrics"
        || s.starts_with("scorestream.lyrics.")
}
pub fn encode(l: &Lyric, event: &str, index: usize) -> Mark {
    let mut a = BTreeMap::from([
        (
            "index".into(),
            Scalar::Integer(i64::try_from(index).expect("representable lyric index")),
        ),
        ("syllabic".into(), Scalar::String(l.syllabic.name().into())),
        ("text".into(), Scalar::String(l.text.clone())),
    ]);
    for (k, v) in [("number", &l.number), ("color", &l.color)] {
        if let Some(v) = v {
            a.insert(k.into(), Scalar::String(v.clone()));
        }
    }
    for (k, v) in [
        ("default_x", &l.default_x),
        ("default_y", &l.default_y),
        ("relative_x", &l.relative_x),
        ("relative_y", &l.relative_y),
    ] {
        if let Some(v) = v {
            a.insert(k.into(), Scalar::String(v.to_string()));
        }
    }
    Mark {
        target: format!("#{event}"),
        kind: KIND.into(),
        attributes: a,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<EventLyrics>, Vec<Diagnostic>> {
    let fail = |source| {
        vec![Diagnostic::error(
            "lyrics.wire",
            "Lyrics require known closed fields, exact event onset, contiguous per-event indices, and scorestream.lyrics@1 on a complete score",
            source,
        )]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let events: BTreeMap<_, _> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| {
                if let Record::Event(e) = r {
                    e.id().map(|id| (id, &f.time))
                } else {
                    None
                }
            })
        })
        .collect();
    let mut groups = BTreeMap::<String, BTreeMap<usize, Lyric>>::new();
    for frame in &doc.frames {
        for r in &frame.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            let err = || fail(m.source);
            if !enabled || doc.document_kind != DocumentKind::Score || m.kind != KIND {
                return Err(err());
            }
            let event = m.target.strip_prefix('#').ok_or_else(err)?;
            if events.get(event) != Some(&&frame.time) {
                return Err(err());
            }
            let a = &m.attributes;
            if a.keys().any(|k| {
                ![
                    "index",
                    "syllabic",
                    "text",
                    "number",
                    "color",
                    "default_x",
                    "default_y",
                    "relative_x",
                    "relative_y",
                ]
                .contains(&k.as_str())
            }) {
                return Err(err());
            }
            let string = |key: &str| {
                a.get(key)
                    .map(|v| v.as_str().map(str::to_string).ok_or_else(err))
                    .transpose()
            };
            let rational = |key: &str| -> Result<Option<Rational>, Vec<Diagnostic>> {
                string(key)?
                    .map(|s| s.parse().map_err(|_| err()))
                    .transpose()
            };
            let index = a
                .get("index")
                .and_then(Scalar::as_i64)
                .and_then(|i| usize::try_from(i).ok())
                .ok_or_else(err)?;
            let l = Lyric {
                number: string("number")?,
                syllabic: a
                    .get("syllabic")
                    .and_then(Scalar::as_str)
                    .and_then(Syllabic::from_name)
                    .ok_or_else(err)?,
                text: string("text")?.ok_or_else(err)?,
                color: string("color")?,
                default_x: rational("default_x")?,
                default_y: rational("default_y")?,
                relative_x: rational("relative_x")?,
                relative_y: rational("relative_y")?,
            };
            if !l.valid()
                || groups
                    .entry(event.into())
                    .or_default()
                    .insert(index, l)
                    .is_some()
            {
                return Err(err());
            }
        }
    }
    if enabled && groups.is_empty() {
        return Err(fail(doc.source));
    }
    let mut result = vec![];
    for (event, lyrics) in groups {
        if lyrics.keys().copied().ne(0..lyrics.len()) {
            return Err(fail(doc.source));
        }
        result.push(EventLyrics {
            event,
            lyrics: lyrics.into_values().collect(),
        });
    }
    Ok(result)
}
pub fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    if let Err(mut errors) = decode(doc) {
        out.append(&mut errors);
    }
}
