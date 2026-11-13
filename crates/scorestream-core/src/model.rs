use crate::{Rational, RationalError};
use num_bigint::BigInt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeMap, fmt, str::FromStr};

pub type Attributes = BTreeMap<String, Scalar>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    String(String),
    Integer(i64),
    Boolean(bool),
}
impl Scalar {
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Boolean(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
impl SourceRange {
    pub fn line(line: usize, length: usize) -> Self {
        Self {
            line,
            column: 1,
            end_line: line,
            end_column: length + 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub source: SourceRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}
impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>, source: SourceRange) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            source,
            object_id: None,
            suggestion: None,
        }
    }
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        source: SourceRange,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, message, source)
        }
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.source.line, self.source.column, self.code, self.message
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentKind {
    Score,
    Fragment,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format: String,
    pub format_version: String,
    pub document_kind: DocumentKind,
    pub time_unit: String,
    pub parts: Vec<Part>,
    pub voices: Vec<Voice>,
    pub metadata: Attributes,
    pub required_extensions: Vec<String>,
    pub frames: Vec<Frame>,
    pub end: Rational,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Rational>,
    #[serde(skip)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip)]
    pub source: SourceRange,
}
impl Document {
    pub fn new(document_kind: DocumentKind) -> Self {
        Self {
            format: "ScoreStream".into(),
            format_version: "1.1".into(),
            document_kind,
            time_unit: "quarter".into(),
            parts: vec![],
            voices: vec![],
            metadata: Attributes::new(),
            required_extensions: vec![],
            frames: vec![],
            end: Rational::zero(),
            span: None,
            diagnostics: vec![],
            source: SourceRange::default(),
        }
    }
    pub fn events(&self) -> impl Iterator<Item = (&Rational, &Event)> {
        self.frames.iter().flat_map(|frame| {
            frame.records.iter().filter_map(move |record| match record {
                Record::Event(event) => Some((&frame.time, event)),
                _ => None,
            })
        })
    }
    pub fn event_count(&self) -> usize {
        self.events().count()
    }
    /// Exact half-open interval query. Grace notes never consume measured time.
    pub fn active_events<'a>(&'a self, at: &'a Rational) -> impl Iterator<Item = &'a Event> {
        self.events().filter_map(move |(start, event)| {
            event
                .duration
                .measured()
                .filter(|d| start <= at && at < &(start + *d))
                .map(|_| event)
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub instrument: String,
    pub staves: u32,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub id: String,
    pub part: String,
    pub staff: u32,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub time: Rational,
    pub records: Vec<Record>,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "lowercase")]
pub enum Record {
    Event(Event),
    State(State),
    Measure(Measure),
    Mark(Mark),
    Relation(Relation),
}
impl Record {
    pub fn source(&self) -> SourceRange {
        match self {
            Self::Event(v) => v.source,
            Self::State(v) => v.source,
            Self::Measure(v) => v.source,
            Self::Mark(v) => v.source,
            Self::Relation(v) => v.source,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub voice: String,
    pub pitch: Pitch,
    pub duration: Duration,
    pub attributes: Attributes,
    #[serde(skip)]
    pub source: SourceRange,
}
impl Event {
    pub fn id(&self) -> Option<&str> {
        self.attributes.get("id").and_then(Scalar::as_str)
    }
    pub fn chord(&self) -> Option<&str> {
        self.attributes.get("chord").and_then(Scalar::as_str)
    }
    pub fn is_grace(&self) -> bool {
        self.duration == Duration::Grace
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub target: String,
    pub attributes: Attributes,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    pub target: String,
    pub id: String,
    pub length: Rational,
    pub attributes: Attributes,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub target: String,
    pub kind: String,
    pub attributes: Attributes,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationPhase {
    Start,
    Continue,
    End,
}
impl fmt::Display for RelationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Start => "start",
            Self::Continue => "continue",
            Self::End => "end",
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub kind: String,
    pub id: String,
    pub phase: RelationPhase,
    pub anchor: String,
    pub attributes: Attributes,
    #[serde(skip)]
    pub source: SourceRange,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Duration {
    Measured(Rational),
    Grace,
}
impl Duration {
    pub fn measured(&self) -> Option<&Rational> {
        if let Self::Measured(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured(v) => v.fmt(f),
            Self::Grace => f.write_str("?"),
        }
    }
}
impl FromStr for Duration {
    type Err = RationalError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "?" {
            return Ok(Self::Grace);
        }
        let value: Rational = s.parse()?;
        if !value.is_positive() {
            return Err(RationalError {
                code: "duration_not_positive",
                message: "Measured event durations must be positive".into(),
            });
        }
        Ok(Self::Measured(value))
    }
}
impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Written pitch retains spelling; numeric pitch is a derived value, not identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pitch {
    Pitched {
        step: char,
        alteration: Rational,
        octave: BigInt,
    },
    Unpitched(String),
    Rest,
    Spacer,
}
impl Pitch {
    pub fn is_note(&self) -> bool {
        matches!(self, Self::Pitched { .. } | Self::Unpitched(_))
    }
    pub fn semitones(&self) -> Option<Rational> {
        if let Self::Pitched {
            step,
            alteration,
            octave,
        } = self
        {
            let offset = match step {
                'C' => 0,
                'D' => 2,
                'E' => 4,
                'F' => 5,
                'G' => 7,
                'A' => 9,
                'B' => 11,
                _ => return None,
            };
            Some(&Rational::from_big_integer(octave * 12 + offset) + alteration)
        } else {
            None
        }
    }
}
impl FromStr for Pitch {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "r" {
            return Ok(Self::Rest);
        }
        if s == "_" {
            return Ok(Self::Spacer);
        }
        if let Some(id) = s.strip_prefix("u.") {
            return if is_identifier(id) {
                Ok(Self::Unpitched(id.into()))
            } else {
                Err("Invalid unpitched identity".into())
            };
        }
        let Some(step) = s.chars().next() else {
            return Err("Missing pitch".into());
        };
        if !('A'..='G').contains(&step) {
            return Err("Pitch step must be A through G".into());
        }
        let mut tail = &s[1..];
        let alteration = if tail.starts_with('(') {
            let close = tail.find(')').ok_or("Unclosed pitch alteration")?;
            let a = tail[1..close].strip_prefix('+').unwrap_or(&tail[1..close]);
            let value = a.parse::<Rational>().map_err(|e| e.to_string())?;
            tail = &tail[close + 1..];
            value
        } else {
            let mut value = 0;
            for (token, n) in [("##", 2), ("bb", -2), ("#", 1), ("b", -1)] {
                if tail.starts_with(token) {
                    value = n;
                    tail = &tail[token.len()..];
                    break;
                }
            }
            Rational::from_integer(value)
        };
        let digits = tail.strip_prefix('-').unwrap_or(tail);
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
            || tail == "-0"
        {
            return Err("Pitch requires a canonical signed integer octave".into());
        }
        Ok(Self::Pitched {
            step,
            alteration,
            octave: tail.parse().map_err(|_| "Invalid octave")?,
        })
    }
}
impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rest => f.write_str("r"),
            Self::Spacer => f.write_str("_"),
            Self::Unpitched(v) => write!(f, "u.{v}"),
            Self::Pitched {
                step,
                alteration,
                octave,
            } => {
                write!(f, "{step}")?;
                match alteration.to_string().as_str() {
                    "0" => (),
                    "1" => f.write_str("#")?,
                    "2" => f.write_str("##")?,
                    "-1" => f.write_str("b")?,
                    "-2" => f.write_str("bb")?,
                    a => write!(
                        f,
                        "({}{a})",
                        if alteration.is_positive() { "+" } else { "" }
                    )?,
                }
                write!(f, "{octave}")
            }
        }
    }
}
impl Serialize for Pitch {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Pitch {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
pub fn is_identifier(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Entry context is supplied by the caller, never inferred from fragment rows.
/// Authorization of these stable bindings belongs to the application/edit layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FragmentContext {
    pub bindings: Vec<FragmentBinding>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FragmentBinding {
    pub alias: String,
    pub part: String,
    pub staff: u32,
    pub staves: u32,
}
