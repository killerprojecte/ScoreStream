use scorestream_codec::{SourceRange, notation::Origin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealizationEffect {
    Policy,
    NotationOnly,
    Omitted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceDiagnostic {
    pub code: String,
    pub feature: String,
    pub effect: RealizationEffect,
    pub object_id: String,
    pub source: SourceRange,
    pub policy: String,
    pub message: String,
}
#[derive(Default)]
pub(super) struct Diagnostics(BTreeMap<(String, String), PerformanceDiagnostic>);
impl Diagnostics {
    pub fn add(
        &mut self,
        code: &str,
        feature: &str,
        effect: RealizationEffect,
        origin: &Origin,
        message: &str,
    ) {
        self.0.insert(
            (code.into(), origin.object_id.clone()),
            PerformanceDiagnostic {
                code: code.into(),
                feature: feature.into(),
                effect,
                object_id: origin.object_id.clone(),
                source: origin.source,
                policy: "sfs1.1-expression-v2".into(),
                message: message.into(),
            },
        );
    }
    pub fn finish(self) -> (Vec<String>, Vec<PerformanceDiagnostic>) {
        let diagnostics: Vec<_> = self.0.into_values().collect();
        let warnings = diagnostics
            .iter()
            .filter(|d| {
                d.effect != RealizationEffect::NotationOnly || d.code == "words_not_realized"
            })
            .map(|d| format!("{} [{}]: {}", d.code, d.object_id, d.message))
            .collect();
        (warnings, diagnostics)
    }
}
