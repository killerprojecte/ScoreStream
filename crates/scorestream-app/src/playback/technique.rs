//! Opt-in attack techniques. This opaque wrapper is not a browser/default plan.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PizzicatoSoundPolicy {
    GeneralMidiSharedStringsApproximation,
}
/// Only the source-qualified compiler can assign attacks. Instrument identity,
/// timing and tie consolidation remain unchanged; no route is selected here.
#[derive(Clone, Debug)]
pub struct TechniquePlaybackPlan {
    source_sha256: String,
    practice_nonrealizations: Vec<String>,
    plan: PlaybackPlan,
    pizzicato: BTreeSet<String>,
}
impl TechniquePlaybackPlan {
    pub fn practice_nonrealizations(&self) -> &[String] {
        &self.practice_nonrealizations
    }
    /// Exact source bytes consumed by the source-qualified compiler.
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub fn plan(&self) -> &PlaybackPlan {
        &self.plan
    }
    pub fn pizzicato_attacks(&self) -> &BTreeSet<String> {
        &self.pizzicato
    }
}
fn finish(
    source: &str,
    mut plan: PlaybackPlan,
    pizzicato: BTreeSet<String>,
) -> TechniquePlaybackPlan {
    plan.warnings.push("Explicit GM shared-strings pizzicato sample approximation requested; native bank/preset/key verification is still required. No browser or default-renderer substitution is authorized.".into());
    TechniquePlaybackPlan {
        practice_nonrealizations: Vec::new(),
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        plan,
        pizzicato,
    }
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
    Ok(finish(source, plan, attacks))
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
    Ok(finish(source, plan, attacks))
}

#[cfg(test)]
mod source_binding_tests {
    use super::*;
    #[test]
    fn technique_witness_binds_exact_source_bytes() {
        let s = "!score 1.1 unit=q\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=a\n.\n!end at=1\n";
        let a = plan_source_with_pizzicato(
            s,
            PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
        )
        .unwrap();
        let b = plan_source_with_pizzicato(
            &format!("{s}\n"),
            PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
        )
        .unwrap();
        assert_eq!(
            a.source_sha256(),
            format!("{:x}", Sha256::digest(s.as_bytes()))
        );
        assert_ne!(a.source_sha256(), b.source_sha256());
        assert_eq!(a.plan().notes.len(), b.plan().notes.len());
    }
}

/// Closed rehearsal-only path, never selected by ordinary playback. The native
/// consumer must match its sealed local MIDI receipt to the exact source digest.
#[allow(clippy::too_many_arguments)]
pub fn plan_source_with_practice_joint_policies_and_pizzicato(
    source: &str,
    practice: scorestream_codec::notation::PracticeDiscoveryPolicy,
    routes: &[scorestream_codec::notation::PracticeMidiSelection],
    articulation: ExplicitArticulationPolicy,
    grace: &ExplicitGracePolicy,
    grace_sound: GraceSoundPolicy,
    holds: &scorestream_codec::fermata_timing::ExplicitSynchronizedFermataPolicy,
    fermata_sound: FermataSoundPolicy,
    pizzicato: PizzicatoSoundPolicy,
) -> Result<TechniquePlaybackPlan, String> {
    let _ = (fermata_sound, pizzicato);
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    let q = scorestream_codec::notation::perform_document_practice_joint(
        &doc,
        practice,
        routes,
        articulation,
        grace,
        grace_sound,
        holds,
    )
    .map_err(format_diagnostics)?;
    let joint = q.joint();
    let p = joint.performed();
    let mut attacks = BTreeSet::new();
    let plan = compile_with_technique(
        &p.score,
        &doc,
        Some(p),
        Some(joint.timing()),
        Some(GraceView::Combined(joint)),
        Some(&mut attacks),
    )?;
    let mut result = finish(source, plan, attacks);
    for (family, message) in [
        (
            "wedge",
            "No authored crescendo/diminuendo trajectory is realized.",
        ),
        (
            "dashes",
            "No acoustic continuation or termination is inferred.",
        ),
        (
            "ornaments",
            "No auxiliary pitches, repetitions or ornament timing is realized.",
        ),
        ("slur", "No legato overlap, phrasing or bowing is inferred."),
    ] {
        result
            .practice_nonrealizations
            .push(format!("{family}: {message}"));
        result.plan.warnings.push(format!(
            "Explicit rehearsal nonrealization: {family}: {message}"
        ));
        result.plan.diagnostics.push(PerformanceDiagnostic {
            code: format!("practice.{family}_unrepresented"),
            feature: family.into(),
            effect: RealizationEffect::Omitted,
            object_id: "document".into(),
            source: doc.source,
            policy: "written_notes_without_wedge_dashes_ornaments_slur_v1".into(),
            message: message.into(),
        });
    }
    result.plan.diagnostics.push(PerformanceDiagnostic {code:"practice.native_receipt_required".into(),feature:"practice.midi".into(),effect:RealizationEffect::Policy,object_id:"document".into(),source:doc.source,policy:"initial_gm_cc7_centered_v1".into(),message:"Rehearsal only; matching source-bound local MIDI mixer and explicit palette verification are mandatory. External MIDI addresses are retained, not delivered.".into()});
    Ok(result)
}
