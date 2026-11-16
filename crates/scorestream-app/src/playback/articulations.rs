//! Source-qualified authored articulation realization under an explicit performer policy.
use super::{PlaybackPlan, compile_performed, format_diagnostics};
pub use scorestream_codec::notation::ExplicitArticulationPolicy;
use scorestream_codec::notation::{
    PerformanceLimits, QualifiedArticulationPerformance,
    perform_document_with_explicit_articulations,
};

#[derive(Clone, Debug)]
pub struct ExplicitArticulationPlayback {
    plan: PlaybackPlan,
    performance: QualifiedArticulationPerformance,
}
impl ExplicitArticulationPlayback {
    pub fn plan(&self) -> &PlaybackPlan {
        &self.plan
    }
    pub fn performance(&self) -> &QualifiedArticulationPerformance {
        &self.performance
    }
}
pub fn plan_source_with_explicit_articulations(
    source: &str,
    policy: ExplicitArticulationPolicy,
) -> Result<ExplicitArticulationPlayback, String> {
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    let performance =
        perform_document_with_explicit_articulations(&doc, policy, PerformanceLimits::default())
            .map_err(format_diagnostics)?;
    let mut plan = compile_performed(
        &performance.performed().score,
        &doc,
        Some(performance.performed()),
        None,
    )?;
    let message = "Explicit SimpleStaccatoAccentV1 performer choice: staccato half gate, accent +16 velocity steps (clamped); authored styling and topology retained unchanged, not interpreted as sound.";
    plan.warnings.push(message.into());
    plan.diagnostics.push(super::PerformanceDiagnostic {
        code: "articulation.explicit_policy".into(),
        feature: "articulation.authored".into(),
        effect: super::RealizationEffect::Policy,
        object_id: "score".into(),
        source: doc.source,
        policy: "explicit-simple-staccato-accent-v1".into(),
        message: message.into(),
    });
    Ok(ExplicitArticulationPlayback { plan, performance })
}
