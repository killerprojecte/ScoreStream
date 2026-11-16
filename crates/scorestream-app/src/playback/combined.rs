//! Actual playback for the opaque, source-qualified combined interpretation.
use super::{FermataSoundPolicy, PlaybackPlan, compile_combined, format_diagnostics};
use scorestream_codec::{
    Rational,
    fermata_timing::{ExplicitFermataPolicy, SeekPosition, TimingLimits, TimingProjection},
    notation::*,
};

#[derive(Clone, Debug)]
pub struct ExplicitCombinedPlayback {
    plan: PlaybackPlan,
    performance: QualifiedCombinedPerformance,
    fermata_sound_policy: FermataSoundPolicy,
}
impl ExplicitCombinedPlayback {
    pub fn plan(&self) -> &PlaybackPlan {
        &self.plan
    }
    pub fn timing(&self) -> &TimingProjection {
        self.performance.timing()
    }
    pub fn grace_windows(&self) -> &[GraceSoundWindow] {
        self.performance.grace_windows()
    }
    pub fn grace_discovery(&self) -> &GraceDiscovery {
        self.performance.grace_discovery()
    }
    pub fn grace_groups(&self) -> &[GraceGroupRealization] {
        self.performance.grace_groups()
    }
    pub fn grace_sound_policy(&self) -> GraceSoundPolicy {
        self.performance.grace_sound_policy()
    }
    pub fn fermata_sound_policy(&self) -> FermataSoundPolicy {
        self.fermata_sound_policy
    }
    pub fn seek(&self, seconds: &Rational) -> SeekPosition {
        self.performance.seek(seconds)
    }
}
fn parse(source: &str) -> Result<scorestream_codec::Document, String> {
    scorestream_codec::parse_with_prevalidation(source, |doc| {
        preflight_combined_source(doc, &GraceTimingLimits::default(), &TimingLimits::default())
    })
    .map_err(format_diagnostics)
}
pub fn discover_source_graces_and_fermatas(
    source: &str,
) -> Result<CombinedPerformanceDiscovery, String> {
    discover_document_graces_and_fermatas(&parse(source)?).map_err(format_diagnostics)
}
/// Both allocations and both sound policies must be selected by the caller.
/// No UI dispatch, source rewriting, clock substitution or inferred duration.
pub fn plan_source_with_explicit_graces_and_fermatas(
    source: &str,
    grace_policy: &ExplicitGracePolicy,
    grace_sound_policy: GraceSoundPolicy,
    fermata_policy: &ExplicitFermataPolicy,
    fermata_sound_policy: FermataSoundPolicy,
) -> Result<ExplicitCombinedPlayback, String> {
    let doc = parse(source)?;
    let performance = perform_document_with_explicit_graces_and_fermatas(
        &doc,
        grace_policy,
        grace_sound_policy,
        fermata_policy,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
        TimingLimits::default(),
    )
    .map_err(format_diagnostics)?;
    match fermata_sound_policy {
        FermataSoundPolicy::SustainSelectedAnchors => (),
    }
    let plan = compile_combined(&doc, &performance)?;
    Ok(ExplicitCombinedPlayback {
        plan,
        performance,
        fermata_sound_policy,
    })
}

#[cfg(test)]
mod tests;
