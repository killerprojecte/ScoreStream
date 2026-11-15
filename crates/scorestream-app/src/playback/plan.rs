use super::diagnostic::PerformanceDiagnostic;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvelopePoint {
    pub offset_seconds: f64,
    /// Multiplier relative to the note's initial MIDI velocity, not absolute gain.
    pub gain: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackNote {
    pub id: String,
    /// Authored selection identity; id is unique per performed occurrence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_id: Option<String>,
    pub part: String,
    pub instrument: String,
    pub voice: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub midi_note: f64,
    pub frequency_hz: f64,
    pub velocity: u8,
    pub gain_envelope: Vec<EnvelopePoint>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaybackRoute {
    pub part: String,
    pub instrument: String,
}
/// Written score intervals, independent of audible attacks, gates and pedal release.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackEventPosition {
    /// Authored identity, repeated across performed visits.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<String>,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackMeasure {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_index: Option<usize>,
    pub part: String,
    pub label: Option<String>,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackPlan {
    pub duration_seconds: f64,
    pub notes: Vec<PlaybackNote>,
    #[serde(default)]
    pub event_positions: Vec<PlaybackEventPosition>,
    #[serde(default)]
    pub measures: Vec<PlaybackMeasure>,
    pub routes: Vec<PlaybackRoute>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<PerformanceDiagnostic>,
}
