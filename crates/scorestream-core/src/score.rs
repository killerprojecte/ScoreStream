//! Format-neutral authoring model for the explicitly supported conversion profile.
//! Not the SFS wire AST. No foreign markup, renderer payloads, or arbitrary maps.
use crate::notation::{EventId, InstrumentId, PartId, VoiceId};
use crate::{DocumentMetadata, Pitch, Rational};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreDocument {
    pub metadata: DocumentMetadata,
    pub parts: Vec<ScorePart>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorePart {
    pub id: PartId,
    pub name: Option<String>,
    pub instrument: Option<InstrumentId>,
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
