//! Strict adapters through ScoreDocument. No adapter passes markup to renderers.
#![forbid(unsafe_code)]
pub mod musicxml;
pub mod sfs;
mod validate;
use scorestream_core::{SourceRange, score::ScoreDocument};
use serde::{Deserialize, Serialize};
pub use validate::validate_score;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Sfs,
    MusicXml,
    ScoreModel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingEffect {
    Unsupported,
    Invalid,
    ResourceLimit,
    Normalized,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingDiagnostic {
    pub format: Format,
    pub effect: MappingEffect,
    pub feature: String,
    pub path: String,
    pub message: String,
    pub source: Option<SourceRange>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingReport {
    pub diagnostics: Vec<MappingDiagnostic>,
}
impl MappingReport {
    pub fn push(
        &mut self,
        format: Format,
        effect: MappingEffect,
        feature: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(MappingDiagnostic {
            format,
            effect,
            feature: feature.into(),
            path: path.into(),
            message: message.into(),
            source: None,
        });
    }
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.effect != MappingEffect::Normalized)
    }
    pub fn finish<T>(self, value: T) -> Result<Conversion<T>, Self> {
        if self.has_errors() {
            Err(self)
        } else {
            Ok(Conversion {
                value,
                report: self,
            })
        }
    }
}
#[derive(Clone, Debug)]
pub struct Conversion<T> {
    pub value: T,
    pub report: MappingReport,
}
/// Adding a format means implementing an explicit mapping, not registering a renderer.
pub trait ScoreAdapter {
    fn read(&self, source: &str) -> Result<Conversion<ScoreDocument>, MappingReport>;
    fn write(&self, score: &ScoreDocument) -> Result<Conversion<String>, MappingReport>;
}
