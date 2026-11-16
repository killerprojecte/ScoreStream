//! Opt-in attack techniques. This opaque wrapper is not a browser/default plan.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PizzicatoSoundPolicy {
    GeneralMidiSharedStringsApproximation,
}
/// Only the source-qualified compiler can assign attacks. Instrument identity,
/// timing and tie consolidation remain unchanged; no route is selected here.
#[derive(Clone, Debug)]
pub struct TechniquePlaybackPlan {
    plan: PlaybackPlan,
    pizzicato: BTreeSet<String>,
}
impl TechniquePlaybackPlan {
    pub fn plan(&self) -> &PlaybackPlan {
        &self.plan
    }
    pub fn pizzicato_attacks(&self) -> &BTreeSet<String> {
        &self.pizzicato
    }
}
fn finish(mut plan: PlaybackPlan, pizzicato: BTreeSet<String>) -> TechniquePlaybackPlan {
    plan.warnings.push("Explicit GM shared-strings pizzicato sample approximation requested; native bank/preset/key verification is still required. No browser or default-renderer substitution is authorized.".into());
    TechniquePlaybackPlan { plan, pizzicato }
}
pub fn plan_source_with_pizzicato(
    source: &str,
    policy: PizzicatoSoundPolicy,
) -> Result<TechniquePlaybackPlan, String> {
    let _ = policy;
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    let mut attacks = BTreeSet::new();
    let plan = if doc
        .required_extensions
        .iter()
        .any(|e| e == scorestream_codec::navigation_extension::EXTENSION)
    {
        let p = perform_document(&doc, PerformanceLimits::default()).map_err(format_diagnostics)?;
        compile_with_technique(&p.score, &doc, Some(&p), None, None, Some(&mut attacks))?
    } else {
        let score = lower_document(&doc).map_err(format_diagnostics)?;
        compile_with_technique(&score, &doc, None, None, None, Some(&mut attacks))?
    };
    Ok(finish(plan, attacks))
}
/// All existing articulation, grace and synchronized hold choices remain explicit.
#[allow(clippy::too_many_arguments)]
pub fn plan_source_with_joint_policies_and_pizzicato(
    source: &str,
    articulation_policy: ExplicitArticulationPolicy,
    grace_policy: &ExplicitGracePolicy,
    grace_sound_policy: GraceSoundPolicy,
    fermata_policy: &scorestream_codec::fermata_timing::ExplicitSynchronizedFermataPolicy,
    fermata_sound_policy: FermataSoundPolicy,
    policy: PizzicatoSoundPolicy,
) -> Result<TechniquePlaybackPlan, String> {
    let _ = (policy, fermata_sound_policy);
    let doc = scorestream_codec::parse_with_prevalidation(source, |d| {
        preflight_combined_source(
            d,
            &GraceTimingLimits::default(),
            &scorestream_codec::fermata_timing::TimingLimits::default(),
        )
    })
    .map_err(format_diagnostics)?;
    let q = perform_document_with_explicit_joint_policies(
        &doc,
        articulation_policy,
        grace_policy,
        grace_sound_policy,
        fermata_policy,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
        scorestream_codec::fermata_timing::TimingLimits::default(),
    )
    .map_err(format_diagnostics)?;
    let mut attacks = BTreeSet::new();
    let p = q.performed();
    let plan = compile_with_technique(
        &p.score,
        &doc,
        Some(p),
        Some(q.timing()),
        Some(GraceView::Combined(&q)),
        Some(&mut attacks),
    )?;
    Ok(finish(plan, attacks))
}
