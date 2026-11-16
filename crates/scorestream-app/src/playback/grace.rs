//! Explicit imported-grace realization into the real immutable playback plan.
use super::{PlaybackPlan, compile_scheduled, format_diagnostics};
pub use scorestream_codec::notation::GraceSoundPolicy;
use scorestream_codec::{Rational, notation::*};

#[derive(Clone, Debug)]
pub struct ExplicitGracePlayback {
    plan: PlaybackPlan,
    schedule: GraceSchedule,
}
impl ExplicitGracePlayback {
    pub fn plan(&self) -> &PlaybackPlan {
        &self.plan
    }
    pub fn schedule(&self) -> &GraceSchedule {
        &self.schedule
    }
    pub fn sound_policy(&self) -> GraceSoundPolicy {
        self.schedule.sound_policy()
    }
    /// Exact score position. Sound windows, rather than zero-length written
    /// grace markers, determine which attacks a transport may resume.
    pub fn seek(&self, seconds: &Rational) -> Rational {
        self.schedule.seek(seconds)
    }
}
fn parse(source: &str) -> Result<scorestream_codec::Document, String> {
    scorestream_codec::parse_with_prevalidation(source, |doc| {
        preflight_grace_source(doc, &GraceTimingLimits::default())
    })
    .map_err(format_diagnostics)
}
pub fn discover_source_graces(source: &str) -> Result<GraceDiscovery, String> {
    discover_document_graces(&parse(source)?).map_err(format_diagnostics)
}
pub fn plan_source_with_explicit_graces(
    source: &str,
    policy: &ExplicitGracePolicy,
    sound_policy: GraceSoundPolicy,
) -> Result<ExplicitGracePlayback, String> {
    let doc = parse(source)?;
    let qualified = perform_document_with_explicit_graces(
        &doc,
        policy,
        sound_policy,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
    )
    .map_err(format_diagnostics)?;
    let performed = qualified.performed();
    let plan = compile_scheduled(
        &performed.score,
        &doc,
        Some(performed),
        None,
        Some(qualified.schedule()),
    )?;
    Ok(ExplicitGracePlayback {
        plan,
        schedule: qualified.into_schedule(),
    })
}

#[cfg(test)]
mod tests;
