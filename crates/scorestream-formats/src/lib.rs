//! Strict adapters through ScoreDocument. No adapter passes markup to renderers.
#![forbid(unsafe_code)]
mod accidentals;
mod articulations;
mod beams;
mod dynamics;
mod fermatas;
mod graces;
pub mod musicxml;
pub mod mxl;
pub mod navigation;
pub mod sfs;
mod sfs_navigation;
mod slurs;
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
    /// A deliberate caller policy was applied; its scope and any permitted loss are reported.
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
    /// Number of identical preservation notices; path/source identify the first occurrence.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub occurrences: u64,
}
fn one() -> u64 {
    1
}
fn is_one(n: &u64) -> bool {
    *n == 1
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
        self.insert(MappingDiagnostic {
            format,
            effect,
            feature: feature.into(),
            path: path.into(),
            message: message.into(),
            source,
            occurrences: 1,
        });
    }
    /// Compact an explicitly repeated preservation receipt, retaining its first source sample.
    /// Typed source data is not coalesced. Failures must use ordinary push_source.
    pub(crate) fn preserve_repeated(
        &mut self,
        format: Format,
        feature: &str,
        path: &str,
        message: &str,
        source: SourceRange,
    ) {
        if let Some(old) = self.diagnostics.iter_mut().find(|d| {
            d.format == format
                && d.effect == MappingEffect::Preserved
                && d.feature == feature
                && d.message == message
        }) {
            old.occurrences = old
                .occurrences
                .checked_add(1)
                .expect("receipt count exceeds addressable input");
        } else {
            self.push_source(
                format,
                MappingEffect::Preserved,
                feature,
                path,
                message,
                Some(source),
            );
        }
    }
    fn insert(&mut self, mut diagnostic: MappingDiagnostic) {
        diagnostic.feature = bounded(diagnostic.feature, 512);
        diagnostic.path = bounded(diagnostic.path, 4096);
        diagnostic.message = bounded(diagnostic.message, 4096);
        const LIMIT: usize = 1024;
        if self.diagnostics.len() >= LIMIT - 1 {
            // Keep a concrete first error even when informational notices filled the budget.
            // The final truncation marker still records error severity for omitted failures.
            if is_error(diagnostic.effect)
                && !self
                    .diagnostics
                    .iter()
                    .any(|d| d.feature != "diagnostics.truncated" && is_error(d.effect))
            {
                self.diagnostics[LIMIT - 2] = diagnostic.clone();
            }
            if self.diagnostics.len() == LIMIT - 1 {
                self.diagnostics.push(MappingDiagnostic {
                    format: diagnostic.format, effect: diagnostic.effect,
                    feature: "diagnostics.truncated".into(), path: "/".into(),
                    message: "Additional diagnostics omitted; this limits the receipt, not source acceptance. Any error severity and first concrete error are retained".into(),
                    source: None, occurrences: 1,
                });
            } else if is_error(diagnostic.effect) {
                self.diagnostics[LIMIT - 1].effect = diagnostic.effect;
            }
            return;
        }
        self.diagnostics.push(diagnostic);
    }
    pub fn append(&mut self, other: Self) {
        for diagnostic in other.diagnostics {
            // Aggregate receipts retain exact counts and their first source through merges.
            if diagnostic.effect == MappingEffect::Preserved {
                if let Some(old) = self.diagnostics.iter_mut().find(|d| {
                    d.format == diagnostic.format
                        && d.effect == diagnostic.effect
                        && d.feature == diagnostic.feature
                        && d.message == diagnostic.message
                        && (d.occurrences > 1 || diagnostic.occurrences > 1)
                }) {
                    old.occurrences = old
                        .occurrences
                        .checked_add(diagnostic.occurrences)
                        .expect("receipt count exceeds addressable input");
                    continue;
                }
            }
            self.insert(diagnostic);
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
fn is_error(effect: MappingEffect) -> bool {
    !matches!(
        effect,
        MappingEffect::Normalized | MappingEffect::PolicyApplied | MappingEffect::Preserved
    )
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
