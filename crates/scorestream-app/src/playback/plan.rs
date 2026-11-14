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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackPlan {
    pub duration_seconds: f64,
    pub notes: Vec<PlaybackNote>,
    pub routes: Vec<PlaybackRoute>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<PerformanceDiagnostic>,
}
