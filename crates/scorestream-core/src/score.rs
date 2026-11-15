//! Format-neutral authoring model for the explicitly supported conversion profile.
//! Not the SFS wire AST. No foreign markup, renderer payloads, or arbitrary maps.
use crate::notation::{EventId, InstrumentId, PartId, RelationId, VoiceId};
use crate::{DocumentMetadata, Pitch, Rational};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreDocument {
    pub metadata: DocumentMetadata,
    pub parts: Vec<ScorePart>,
    /// Ordinary printed and sounding ties. Written notes remain separate events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ties: Vec<ScoreTie>,
    /// Authored printed slurs with independent endpoint geometry. Playback is unsupported.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slurs: Vec<ScoreSlur>,
    /// Explicit, score-wide sounding quarter-note BPM, independent of printed marks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempos: Vec<ScoreTempo>,
    /// Printed numeric metronome marks. These never implicitly change playback tempo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metronomes: Vec<ScoreMetronome>,
    /// Authored printed symbols, independent of explicit numeric playback suggestions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamics: Vec<ScoreDynamic>,
    /// Exact MusicXML percentages of the standard forte value 90, never label estimates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sound_dynamics: Vec<ScoreSoundDynamics>,
    /// Authored per-part boundary signs. Performance navigation is derived separately.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub barlines: Vec<ScoreBarline>,
    /// Authored note/rest signs only. No hold duration or playback policy is inferred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fermatas: Vec<ScoreFermata>,
    /// Authored unmeasured notes, not zero-length ordinary events or silent cues.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graces: Vec<ScoreGraceNote>,
    /// Authored printed beams. No timing, grouping, or missing levels are inferred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<ScoreBeam>,
}
/// One MusicXML beam value on one event, retaining an absent number separately
/// from an explicitly authored primary level (1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreBeam {
    pub event: EventId,
    pub number: Option<u8>,
    pub value: BeamValue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamValue {
    #[serde(rename = "begin")]
    Begin,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "end")]
    End,
    #[serde(rename = "forward hook")]
    ForwardHook,
    #[serde(rename = "backward hook")]
    BackwardHook,
}
impl BeamValue {
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "begin" => Some(Self::Begin),
            "continue" => Some(Self::Continue),
            "end" => Some(Self::End),
            "forward hook" => Some(Self::ForwardHook),
            "backward hook" => Some(Self::BackwardHook),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Begin => "begin",
            Self::Continue => "continue",
            Self::End => "end",
            Self::ForwardHook => "forward hook",
            Self::BackwardHook => "backward hook",
        }
    }
}
/// One authored fermata on a measured note or rest. The supported profile permits
/// one sign per event and retains every distinct event's sign without coalescing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreFermata {
    pub event: EventId,
    /// None preserves an empty MusicXML fermata, distinct from explicit "normal".
    pub shape: Option<FermataShape>,
    /// None preserves an absent orientation, whose MusicXML default is upright.
    pub orientation: Option<FermataOrientation>,
    pub default_y: Option<Rational>,
    pub relative_y: Option<Rational>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FermataShape {
    Normal,
    Angled,
    Square,
    DoubleAngled,
    DoubleSquare,
    DoubleDot,
    HalfCurve,
    Curlew,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FermataOrientation {
    Upright,
    Inverted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorePartScope {
    pub part: PartId,
    pub staff: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreDynamic {
    pub at: Rational,
    pub scope: ScorePartScope,
    pub sign: crate::notation::DynamicSign,
    pub placement: Option<crate::notation::Placement>,
    /// false for MusicXML printed dynamics; numeric sound is a separate lane.
    pub sound: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreSoundDynamics {
    pub at: Rational,
    pub scope: ScorePartScope,
    pub percent: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreTempo {
    pub at: Rational,
    pub quarter_bpm: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreMetronome {
    pub at: Rational,
    pub part: PartId,
    /// None means a part direction, not an inferred staff or score-wide mark.
    pub staff: Option<u32>,
    pub value: crate::notation::Tempo,
}
/// One authored slur, reusing the native SFS relation identity. Source-local XML
/// numbers are not identity and may be regenerated by an adapter with a report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreSlur {
    pub id: RelationId,
    pub start: SlurEndpoint,
    pub end: SlurEndpoint,
}
/// Independent authored fields on one slur endpoint. Coordinates are exact
/// MusicXML tenths relative to that endpoint, not absolute page coordinates.
/// The start pair is the first Bézier control point; the end pair is the second.
/// Each coordinate may independently be absent. No defaults are inferred.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlurEndpoint {
    pub event: EventId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bezier_x: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bezier_y: Option<Rational>,
    /// Curvature orientation, independent of staff-relative placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<SlurOrientation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<crate::notation::Placement>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlurOrientation {
    Over,
    Under,
}
/// A typed chain of distinct measured notes, with stable notation identity.
/// This conversion profile does not yet represent print-only, sound-only,
/// repeat-conditioned, let-ring, or engraving-continuation ties.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreTie {
    pub id: RelationId,
    pub events: Vec<EventId>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorePart {
    pub id: PartId,
    pub name: Option<String>,
    pub instrument: Option<InstrumentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_metadata: Option<crate::instrument_metadata::InstrumentMetadata>,
    pub staves: u32,
    pub voices: Vec<ScoreVoice>,
    pub measures: Vec<ScoreMeasure>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreVoice {
    pub id: VoiceId,
    pub staff: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreMeasure {
    pub id: String,
    pub label: String,
    pub start: Rational,
    /// Actual elapsed quarter-note extent, independent of the inherited written meter.
    /// An irregular authored bar never rewrites the meter or implies hidden numbering.
    pub length: Rational,
    pub implicit: bool,
    /// Changes at this measure's start. Later measures inherit preceding state.
    pub states: Vec<StaffState>,
    pub events: Vec<ScoreEvent>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaffState {
    pub staff: u32,
    pub clef: Option<Clef>,
    pub key: Option<KeySignature>,
    pub meter: Option<Meter>,
    /// Written-to-sounding transposition; inherited until an explicit replacement.
    /// The written key and written event pitch are never modified by this state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose: Option<Transposition>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transposition {
    pub chromatic: Rational,
    /// MusicXML permits the spelling interval to be absent, not implicitly zero.
    pub diatonic: Option<i64>,
    pub octaves: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClefSign {
    G,
    F,
    C,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clef {
    pub sign: ClefSign,
    pub line: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeySignature {
    pub fifths: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meter {
    pub beats: Vec<u32>,
    pub beat_type: u32,
}
impl Meter {
    pub fn quarters(&self) -> Rational {
        let beats = self.beats.iter().fold(Rational::zero(), |a, n| {
            &a + &Rational::from_integer(i64::from(*n))
        });
        &(&beats * &Rational::from_integer(4)) / &Rational::from_integer(i64::from(self.beat_type))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreEvent {
    pub id: EventId,
    pub voice: VoiceId,
    pub staff: u32,
    /// Exact quarter-note offset from the beginning of the measure.
    pub offset: Rational,
    pub duration: Rational,
    pub pitch: Pitch,
    /// Ordered membership, not a shared duration; each tone has its own duration.
    pub chord: Option<String>,
    pub written: Option<WrittenDuration>,
    /// Authored stem, independent of pitch, written type and sounding duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stem: Option<StemDirection>,
    /// Authored printed glyph and flags, independent of sounding pitch and duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accidental: Option<PrintedAccidental>,
    /// None, explicit no, and yes remain distinct. True spans this actual measure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure_rest: Option<bool>,
}
/// One explicitly authored accidental glyph. Absence does not infer a glyph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintedAccidental {
    pub glyph: AccidentalGlyph,
    /// MusicXML defaults cautionary to no, but its authored absence is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cautionary: Option<bool>,
    /// None leaves enclosure to renderer defaults; explicit false is not absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parentheses: Option<bool>,
}
/// Finite intersection of MusicXML glyphs and the existing SFS accidental registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccidentalGlyph {
    Natural,
    Sharp,
    Flat,
    DoubleSharp,
    FlatFlat,
    QuarterSharp,
    QuarterFlat,
}
impl AccidentalGlyph {
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "natural" => Some(Self::Natural),
            "sharp" => Some(Self::Sharp),
            "flat" => Some(Self::Flat),
            "double-sharp" => Some(Self::DoubleSharp),
            "flat-flat" => Some(Self::FlatFlat),
            "quarter-sharp" => Some(Self::QuarterSharp),
            "quarter-flat" => Some(Self::QuarterFlat),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Sharp => "sharp",
            Self::Flat => "flat",
            Self::DoubleSharp => "double-sharp",
            Self::FlatFlat => "flat-flat",
            Self::QuarterSharp => "quarter-sharp",
            Self::QuarterFlat => "quarter-flat",
        }
    }
}
/// MusicXML / SFS stem values. A stem on a rest is a stemlet, not a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StemDirection {
    Up,
    Down,
    None,
    Double,
}
impl StemDirection {
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "none" => Some(Self::None),
            "double" => Some(Self::Double),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::None => "none",
            Self::Double => "double",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    Maxima,
    Long,
    Breve,
    Whole,
    Half,
    Quarter,
    Eighth,
    N16,
    N32,
    N64,
    N128,
    N256,
    N512,
    N1024,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrittenDuration {
    pub note_type: NoteType,
    /// Graphic size only. Cue size does not create a silent cue or a grace event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<NoteSize>,
    pub dots: u32,
    pub time_modification: Option<TimeModification>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeModification {
    pub actual: u32,
    pub normal: u32,
    pub normal_type: Option<NoteType>,
    pub normal_dots: u32,
}

/// An authored boundary, retained in its original part and measure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreBarline {
    pub part: PartId,
    pub measure: String,
    pub location: BarlineLocation,
    pub id: Option<String>,
    /// None preserves the absence of an explicit line style.
    pub style: Option<BarlineStyle>,
    pub repeat: Option<RepeatSign>,
    pub ending: Option<EndingSign>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarlineLocation {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BarlineStyle {
    Regular,
    Dotted,
    Dashed,
    Heavy,
    LightLight,
    LightHeavy,
    HeavyLight,
    HeavyHeavy,
    Tick,
    Short,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatDirection {
    Backward,
    Forward,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatSign {
    pub direction: RepeatDirection,
    /// Authored count, not an implicitly inserted two-pass convention.
    pub times: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EndingPhase {
    Stop,
    Discontinue,
    Start,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndingSign {
    pub passes: Vec<u32>,
    pub phase: EndingPhase,
    /// Optional display override, independent of the performed pass numbers.
    pub text: String,
    /// Exact authored vertical placement in tenths, not a performance control.
    pub default_y: Option<Rational>,
}

/// MusicXML graphic note size; deliberately independent of event sounding role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteSize {
    Full,
    Cue,
    GraceCue,
    Large,
}
impl NoteSize {
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "full" => Some(Self::Full),
            "cue" => Some(Self::Cue),
            "grace-cue" => Some(Self::GraceCue),
            "large" => Some(Self::Large),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Cue => "cue",
            Self::GraceCue => "grace-cue",
            Self::Large => "large",
        }
    }
}

/// A pitched grace before a measured, non-chord principal in the same voice/staff.
/// Its written position is the principal onset; order -N..-1 is source order.
/// There is deliberately no measured duration. No performance is inferred.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreGraceNote {
    pub id: EventId,
    pub anchor: EventId,
    pub order: i64,
    pub pitch: Pitch,
    pub written: Option<WrittenDuration>,
    /// None, explicit no, and yes remain distinct authored states.
    pub slash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stem: Option<StemDirection>,
    /// Authored printed glyph and flags, independent of sounding pitch and duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accidental: Option<PrintedAccidental>,
    pub timing: GraceTiming,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraceTiming {
    pub steal_previous: Option<Rational>,
    pub steal_following: Option<Rational>,
    /// Exact make-time amount scaled from the active MusicXML divisions.
    /// This is timing intent, not a measured duration or realized seconds.
    pub make_time_quarters: Option<Rational>,
}
impl GraceTiming {
    pub fn valid(&self) -> bool {
        [&self.steal_previous, &self.steal_following]
            .into_iter()
            .flatten()
            .all(|r| !r.is_negative() && *r <= Rational::from_integer(100))
            && self
                .make_time_quarters
                .as_ref()
                .is_none_or(|r| !r.is_negative())
    }
}
