//! Strict adapters through ScoreDocument. No adapter passes markup to renderers.
#![forbid(unsafe_code)]
mod dynamics;
mod fermatas;
mod graces;
pub mod musicxml;
pub mod mxl;
pub mod navigation;
pub mod sfs;
mod sfs_navigation;
mod tempo;
mod ties;
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
    /// A deliberate caller policy was applied without discarding source semantics.
    PolicyApplied,
    /// Typed authored data is retained; any unimplemented realization is stated.
    Preserved,
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
        self.push_source(format, effect, feature, path, message, None);
    }
    /// Reports remain bounded even when a score repeats an unsupported construct many times.
    /// The final summary retains error severity so truncation can never turn failure into success.
    pub(crate) fn push_source(
        &mut self,
        format: Format,
        effect: MappingEffect,
        feature: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        source: Option<SourceRange>,
    ) {
        const LIMIT: usize = 1024;
        if self.diagnostics.len() >= LIMIT - 1 {
            if self.diagnostics.len() == LIMIT - 1 {
                self.diagnostics.push(MappingDiagnostic {
                    format,
                    effect,
                    feature: "diagnostics.truncated".into(),
                    path: "/".into(),
                    message: "Additional diagnostics omitted; fix the reported issues and retry"
                        .into(),
                    source: None,
                });
            } else if !matches!(
                effect,
                MappingEffect::Normalized | MappingEffect::PolicyApplied | MappingEffect::Preserved
            ) {
                self.diagnostics[LIMIT - 1].effect = effect;
            }
            return;
        }
        self.diagnostics.push(MappingDiagnostic {
            format,
            effect,
            feature: bounded(feature.into(), 512),
            path: bounded(path.into(), 4096),
            message: bounded(message.into(), 4096),
            source,
        });
    }
    pub fn append(&mut self, other: Self) {
        for diagnostic in other.diagnostics {
            self.push_source(
                diagnostic.format,
                diagnostic.effect,
                diagnostic.feature,
                diagnostic.path,
                diagnostic.message,
                diagnostic.source,
            );
        }
    }
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| {
            !matches!(
                d.effect,
                MappingEffect::Normalized | MappingEffect::PolicyApplied | MappingEffect::Preserved
            )
        })
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
fn bounded(mut value: String, max: usize) -> String {
    if value.len() > max {
        let mut end = max - 3;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push_str("...");
    }
    value
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
