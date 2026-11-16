//! Opt-in attack techniques. This opaque wrapper is not a browser/default plan.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeNoteExpressionPolicy {
    AuthoredNoteOnWithRelativeTrajectoryV1,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct PracticeNoteDynamicsReceipt {
    pub event: String,
    pub authored_percent: Rational,
    pub exact_note_on: Rational,
    pub integer_velocity: u8,
    pub fractional_gain: f64,
    pub baseline_gain: f64,
    pub relative_trajectory_applied: bool,
    pub original_envelope: Vec<EnvelopePoint>,
    pub realized_envelope: Vec<EnvelopePoint>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeHoldEndpointPolicy {
    ReleaseUnmarkedAtBoundaryBeforeHoldV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeTieNoteOnPolicy {
    InertWithoutNewNoteOnV1,
}
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
    note_dynamics_receipts: Vec<PracticeNoteDynamicsReceipt>,
    plan: PlaybackPlan,
    pizzicato: BTreeSet<String>,
}
impl TechniquePlaybackPlan {
    pub fn note_dynamics_receipts(&self) -> &[PracticeNoteDynamicsReceipt] {
        &self.note_dynamics_receipts
    }
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
        note_dynamics_receipts: Vec::new(),
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
    tie_note_on: PracticeTieNoteOnPolicy,
    endpoint: PracticeHoldEndpointPolicy,
    note_expression: PracticeNoteExpressionPolicy,
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
    let mut note_dynamics_receipts = Vec::new();
    let plan = compile_with_practice(
        &p.score,
        &doc,
        Some(p),
        Some(joint.timing()),
        Some(GraceView::Combined(joint)),
        Some(&mut attacks),
        Some(tie_note_on),
        Some(endpoint),
        Some(note_expression),
        Some(&mut note_dynamics_receipts),
    )?;
    let mut result = finish(source, plan, attacks);
    result.note_dynamics_receipts = note_dynamics_receipts;
    for d in &mut result.plan.diagnostics {
        match d.code.as_str() {
            "practice.tie_continuation_note_on_inert" => {
                d.policy = "inert_without_new_note_on_v1".into()
            }
            "practice.unmarked_endpoint_before_hold" => {
                d.policy = "release_unmarked_at_boundary_before_hold_v1".into()
            }
            "practice.note_on_relative_expression" => {
                d.policy = "authored_note_on_with_relative_trajectory_v1".into()
            }
            _ => (),
        }
    }
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

#[cfg(test)]
mod tied_note_on_tests {
    use super::*;
    fn source(sound: bool, tail: &str) -> String {
        format!(
            "!score 1.1 unit=q\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m len=2\nv C4:1 id=a attack_dynamics=100\n~ tie t start=#a sound={sound}\n.\n@1\nv C4:1 id=b attack_dynamics={tail}\n~ tie t end=#b\n.\n!end at=2\n"
        )
    }
    fn practice(s: &str) -> PlaybackPlan {
        let doc = scorestream_codec::parse(s).unwrap();
        let score = lower_document(&doc).unwrap();
        compile_with_practice(
            &score,
            &doc,
            None,
            None,
            None,
            None,
            Some(PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1),
            None,
            None,
            None,
        )
        .unwrap()
    }
    #[test]
    fn verified_sounding_continuation_note_on_is_inert_not_reattack_or_mute() {
        let s = source(true, "0");
        let before = s.clone();
        assert!(
            plan_source(&s)
                .unwrap_err()
                .contains("tie_continuation_unsupported")
        );
        let p = practice(&s);
        assert_eq!(s, before);
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0].velocity, 90);
        let baseline = plan_source(&s.replace(" id=b attack_dynamics=0", " id=b")).unwrap();
        assert_eq!(
            serde_json::to_value(&p.notes).unwrap(),
            serde_json::to_value(&baseline.notes).unwrap()
        );
        let receipt = p
            .diagnostics
            .iter()
            .find(|d| d.code == "practice.tie_continuation_note_on_inert")
            .unwrap();
        assert_eq!(receipt.object_id, "b");
        assert!(receipt.message.contains("dynamics 0"));
        assert!(receipt.message.contains("predecessor a"));
        let changed = practice(&source(true, "1611/25"));
        assert_eq!(
            serde_json::to_value(p.notes).unwrap(),
            serde_json::to_value(changed.notes).unwrap()
        );
    }
    #[test]
    fn printed_only_tie_does_not_make_fresh_note_on_inert() {
        let s = source(false, "50");
        let p = practice(&s);
        assert_eq!(p.notes.len(), 2);
        assert_eq!(p.notes[1].velocity, 45);
        assert!(
            !p.diagnostics
                .iter()
                .any(|d| d.code == "practice.tie_continuation_note_on_inert")
        );
        assert_eq!(
            serde_json::to_value(p.notes).unwrap(),
            serde_json::to_value(plan_source(&s).unwrap().notes).unwrap()
        );
    }
}

#[cfg(test)]
mod hold_endpoint_tests {
    use super::*;
    use scorestream_codec::fermata_timing::*;
    fn source(duration: &str) -> String {
        format!(
            "!score 1.1 unit=q\n!requires scorestream.fermata@1\n!part p instrument=violin staves=1\n!part r instrument=viola staves=1\n!voice v part=p staff=1\n!voice w part=r staff=1\n@0\n| p m len=2\n| r n len=2\nv C4:1 id=held\nw D4:{duration} id=plain\n+ #held scorestream.fermata orientation=upright\n.\n@1\nv E4:1 id=next\n.\n!end at=2\n"
        )
    }
    fn prepare(s: &str) -> scorestream_codec::notation::QualifiedFermataPerformance {
        let doc = scorestream_codec::parse(s).unwrap();
        let discovery = discover_document_fermatas(&doc).unwrap();
        perform_document_with_explicit_fermatas(
            &doc,
            &ExplicitFermataPolicy {
                groups: vec![ExplicitHoldGroup {
                    id: "h".into(),
                    members: discovery.occurrences.iter().map(|f| f.id.clone()).collect(),
                    added_seconds: Rational::one(),
                }],
            },
            PerformanceLimits::default(),
            TimingLimits::default(),
        )
        .unwrap()
    }
    #[test]
    fn exact_unmarked_endpoint_releases_before_hold_and_next_attack_stays_after() {
        let s = source("1");
        let doc = scorestream_codec::parse(&s).unwrap();
        let q = prepare(&s);
        let p = q.performed();
        assert!(
            compile_with_technique(&p.score, &doc, Some(p), Some(q.timing()), None, None)
                .unwrap_err()
                .contains("fermata.unselected_overlap")
        );
        let plan = compile_with_practice(
            &p.score,
            &doc,
            Some(p),
            Some(q.timing()),
            None,
            None,
            None,
            Some(PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1),
            None,
            None,
        )
        .unwrap();
        let plain = plan
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("plain"))
            .unwrap();
        let held = plan
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("held"))
            .unwrap();
        let next = plan
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("next"))
            .unwrap();
        assert_eq!(plain.duration_seconds, 0.5);
        assert_eq!(plain.gain_envelope.last().unwrap().offset_seconds, 0.5);
        assert_eq!(held.duration_seconds, 1.5);
        assert_eq!(next.start_seconds, 1.5);
        let receipt = plan
            .diagnostics
            .iter()
            .find(|d| d.code == "practice.unmarked_endpoint_before_hold")
            .unwrap();
        assert!(receipt.message.contains("pre-hold seconds 1/2"));
        assert!(receipt.message.contains("after-hold seconds 3/2"));
    }
    #[test]
    fn genuine_unselected_crossing_stays_rejected() {
        let s = source("2");
        let doc = scorestream_codec::parse(&s).unwrap();
        let q = prepare(&s);
        let p = q.performed();
        assert!(
            compile_with_practice(
                &p.score,
                &doc,
                Some(p),
                Some(q.timing()),
                None,
                None,
                None,
                Some(PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1),
                None,
                None
            )
            .unwrap_err()
            .contains("fermata.unselected_overlap")
        );
    }
}

#[cfg(test)]
mod relative_note_expression_tests {
    use super::*;
    fn source(baseline: &str) -> String {
        format!(
            "!score 1.1 unit=q\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m len=2\n= p sound_dynamics={baseline}\nv C4:2 id=a attack_dynamics=1361/25\n.\n@1\n= p sound_dynamics=50\n.\n!end at=2\n"
        )
    }
    #[test]
    fn exact_note_on_and_separate_relative_steps_are_each_applied_once() {
        let s = source("10667/100");
        let before = s.clone();
        let doc = scorestream_codec::parse(&s).unwrap();
        let score = lower_document(&doc).unwrap();
        assert!(
            plan_source(&s)
                .unwrap_err()
                .contains("expression_unsupported")
        );
        let baseline = plan_source(&s.replace(" attack_dynamics=1361/25", "")).unwrap();
        let mut receipts = Vec::new();
        let p = compile_with_practice(
            &score,
            &doc,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1),
            Some(&mut receipts),
        )
        .unwrap();
        assert_eq!(s, before);
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0].velocity, 49);
        assert_eq!(
            p.notes[0].duration_seconds,
            baseline.notes[0].duration_seconds
        );
        let r = &receipts[0];
        assert_eq!(r.exact_note_on, "12249/250".parse().unwrap());
        assert!(r.relative_trajectory_applied);
        assert_eq!(
            serde_json::to_value(&r.original_envelope).unwrap(),
            serde_json::to_value(&baseline.notes[0].gain_envelope).unwrap()
        );
        for (old, new) in r.original_envelope.iter().zip(&r.realized_envelope) {
            assert_eq!(old.offset_seconds, new.offset_seconds);
            assert!((new.gain - r.fractional_gain * old.gain / r.baseline_gain).abs() < 1e-12);
        }
        assert_eq!(
            r.realized_envelope[1].offset_seconds,
            r.realized_envelope[2].offset_seconds
        );
        assert!((49. * r.realized_envelope[0].gain - 48.996).abs() < 1e-12);
        assert!(
            (r.realized_envelope[2].gain / r.realized_envelope[0].gain - 50. / 106.67).abs()
                < 1e-12
        );
    }
    #[test]
    fn zero_onset_relative_baseline_remains_unresolved() {
        let s = source("0");
        let doc = scorestream_codec::parse(&s).unwrap();
        let score = lower_document(&doc).unwrap();
        let e = compile_with_practice(
            &score,
            &doc,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1),
            None,
        )
        .unwrap_err();
        assert!(e.contains("relative_baseline"), "{e}");
    }
}
