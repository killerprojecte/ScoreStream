//! Explicit rehearsal-only integration of the sealed boundary witness.
use super::*;
use scorestream_codec::notation::*;
#[derive(Clone, Debug, serde::Serialize)]
pub struct BoundaryAttackReceipt {
    pub occurrence_id: String,
    pub written_id: String,
    pub part: String,
    pub staff: u32,
    pub exact_percent: Rational,
    pub exact_note_on: Rational,
    pub integer_velocity: u8,
    pub fractional_gain: f64,
    pub terminal_then_opening: [usize; 2],
    pub original_velocity: u8,
    pub original_envelope: Vec<EnvelopePoint>,
    pub realized_envelope: Vec<EnvelopePoint>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct BoundaryCoverageReceipt {
    pub part: String,
    pub staff: u32,
    pub opening_at: Rational,
    pub opening_end: Rational,
    pub until_legacy_or_end: Rational,
    pub post_window_fresh_attacks: usize,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct BoundaryPracticeReceipt {
    pub source_sha256: String,
    pub interpretation_sha256: String,
    pub schedule: AuthoredDynamicSchedule,
    pub attacks: Vec<BoundaryAttackReceipt>,
    pub coverage: Vec<BoundaryCoverageReceipt>,
}
/// Explicit additional rehearsal policy, never called by normal playback.
#[allow(clippy::too_many_arguments)]
pub fn plan_source_with_boundary_practice_joint_policies_and_pizzicato(
    source: &str,
    boundary: PracticeBoundaryAttackPolicy,
    tie_note_on: PracticeTieNoteOnPolicy,
    endpoint: PracticeHoldEndpointPolicy,
    note_expression: PracticeNoteExpressionPolicy,
    practice: PracticeDiscoveryPolicy,
    routes: &[PracticeMidiSelection],
    articulation: ExplicitArticulationPolicy,
    grace: &ExplicitGracePolicy,
    grace_sound: GraceSoundPolicy,
    holds: &scorestream_codec::fermata_timing::ExplicitSynchronizedFermataPolicy,
    fermata_sound: FermataSoundPolicy,
    pizzicato: PizzicatoSoundPolicy,
) -> Result<TechniquePlaybackPlan, String> {
    let _ = (fermata_sound, pizzicato);
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    let q = perform_document_practice_with_boundary_attacks(
        &doc,
        boundary,
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
    let mut pizzicato_attacks = BTreeSet::new();
    let mut note_receipts = Vec::new();
    let mut plan = compile_with_practice(
        &p.score,
        &doc,
        Some(p),
        Some(joint.timing()),
        Some(GraceView::Combined(joint)),
        Some(&mut pizzicato_attacks),
        Some(tie_note_on),
        Some(endpoint),
        Some(note_expression),
        Some(&mut note_receipts),
    )?;
    let events: BTreeMap<_, _> = p.score.events.iter().map(|e| (&e.id.0, e)).collect();
    let visits: BTreeMap<_, _> = p.events.iter().map(|e| (&e.id.0, e)).collect();
    let facts: Vec<_> = plan
        .notes
        .iter()
        .map(|n| {
            let e = events
                .get(&n.id)
                .ok_or("practice.boundary_event: missing actual note event")?;
            let visit = visits
                .get(&n.id)
                .ok_or("practice.boundary_visit: missing event visit")?;
            let measure = p
                .measures
                .iter()
                .find(|m| m.part == e.part && m.occurrence_index == visit.measure_occurrence)
                .ok_or("practice.boundary_owner: missing measure")?;
            Ok(QualifiedPracticeAttack {
                occurrence_id: n.id.clone(),
                written_event_id: visit.written_id.0.clone(),
                part: e.part.0.clone(),
                staff: e.staff.number,
                written_measure: measure.written_id.clone(),
                measure_occurrence: visit.measure_occurrence,
                onset: e.start.clone(),
                chain_attack: e.start.clone(),
                existing_attack_percent: e.attack_dynamics.clone(),
            })
        })
        .collect::<Result<_, String>>()?;
    let resolution = q.resolve_attacks(&facts).map_err(format_diagnostics)?;
    let coverage = audit_coverage(&doc, p, q.schedule(), &facts)?;
    let mut receipts = Vec::new();
    for (fact, decision) in facts.iter().zip(&resolution.decisions) {
        if decision.disposition != PracticeAttackDisposition::Selected {
            if matches!(
                decision.disposition,
                PracticeAttackDisposition::UnverifiedAttackFacts
                    | PracticeAttackDisposition::CompetingStatement
                    | PracticeAttackDisposition::AmbiguousScope
                    | PracticeAttackDisposition::LegacyControlConflict
                    | PracticeAttackDisposition::UnresolvedBoundary
            ) {
                return Err(format!(
                    "practice.boundary_attack_conflict: {} {:?}",
                    fact.occurrence_id, decision.disposition
                ));
            }
            continue;
        }
        let note = plan
            .notes
            .iter_mut()
            .find(|n| n.id == fact.occurrence_id)
            .unwrap();
        if note
            .gain_envelope
            .windows(2)
            .any(|w| w[0].gain != w[1].gain)
        {
            return Err(format!(
                "practice.boundary_changing_envelope: {} requires separate expression semantics; no held trajectory is rewritten",
                note.id
            ));
        }
        let percent = decision.percent.clone().unwrap();
        let exact = &percent * &"9/10".parse::<Rational>().unwrap();
        let value = exact.to_f64();
        if !exact.is_positive()
            || exact > Rational::from_integer(127)
            || !value.is_finite()
            || value == 0.
        {
            return Err("practice.boundary_attack_range: zero/unrepresentable attack needs separate semantics".into());
        }
        let velocity = value.round().clamp(1., 127.) as u8;
        let gain = value / f64::from(velocity);
        let original_envelope = note.gain_envelope.clone();
        let original_velocity = note.velocity;
        // Only this already-identified fresh note is changed. All knot times,
        // attack/release times, route and held chains remain byte-for-byte stable.
        for point in &mut note.gain_envelope {
            point.gain = gain;
        }
        note.velocity = velocity;
        receipts.push(BoundaryAttackReceipt {
            occurrence_id: note.id.clone(),
            written_id: fact.written_event_id.clone(),
            part: fact.part.clone(),
            staff: fact.staff,
            exact_percent: percent,
            exact_note_on: exact,
            integer_velocity: velocity,
            fractional_gain: gain,
            terminal_then_opening: decision.terminal_then_opening.unwrap(),
            original_velocity,
            original_envelope,
            realized_envelope: note.gain_envelope.clone(),
        });
    }
    let receipt = BoundaryPracticeReceipt {
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        interpretation_sha256: format!(
            "{:x}",
            Sha256::digest(scorestream_codec::canonical(q.interpretation_projection()).as_bytes())
        ),
        schedule: q.schedule().clone(),
        attacks: receipts,
        coverage,
    };
    let mut result = finish_practice(source, &doc, plan, pizzicato_attacks, note_receipts)?;
    result.plan.warnings.push(format!("Explicit adjacent-boundary attack-only rehearsal: {} fresh attacks; all {} authored statements retained. No already-sounding voice, tie interval, release, route, legacy state or changing envelope was rewritten.",receipt.attacks.len(),receipt.schedule.written_statements.len()));
    result.plan.diagnostics.push(PerformanceDiagnostic {
        code: "practice.boundary_attack_only".into(),
        feature: "dynamics.authored_boundary".into(),
        effect: RealizationEffect::Policy,
        object_id: "document".into(),
        source: doc.source,
        policy: "adjacent_opening_visit_attack_percent_v1".into(),
        message: result.plan.warnings.last().unwrap().clone(),
    });
    result.boundary_dynamics = Some(receipt);
    Ok(result)
}
fn audit_coverage(
    doc: &Document,
    p: &PerformedScore,
    schedule: &AuthoredDynamicSchedule,
    attacks: &[QualifiedPracticeAttack],
) -> Result<Vec<BoundaryCoverageReceipt>, String> {
    let hosts = scorestream_codec::authored_dynamics::hosts(doc);
    let mut receipts = Vec::new();
    for b in &schedule.boundaries {
        let pair = b
            .terminal_then_opening
            .ok_or("practice.boundary_coverage: unpaired statement")?;
        let o = &schedule.statements[pair[1]];
        let staves = doc
            .parts
            .iter()
            .find(|p| p.id == o.source.part)
            .unwrap()
            .staves;
        for staff in o
            .source
            .staff
            .map(|s| vec![s])
            .unwrap_or_else(|| (1..=staves).collect())
        {
            let mut visits: Vec<_> = p
                .measures
                .iter()
                .filter(|m| m.part.0 == o.source.part && m.start >= o.visit_start)
                .collect();
            visits.sort_by(|a, b| a.start.cmp(&b.start));
            let mut previous_end = None;
            let mut until = &o.visit_start + &o.visit_length;
            for v in visits {
                let (written_start, len, _) = &hosts[&(v.part.0.clone(), v.written_id.clone())];
                if previous_end
                    .as_ref()
                    .is_some_and(|end| end != written_start)
                {
                    return Err("practice.boundary_scope_jump: no carry semantics across a route discontinuity before the next legacy assignment".into());
                }
                let stop=doc.frames.iter().filter(|f|f.time>=*written_start&&f.time<written_start+len).filter(|f|f.time>o.source.sounding_at).find(|f|f.records.iter().any(|r|matches!(r,Record::State(s) if s.attributes.contains_key("sound_dynamics")&&(s.target==o.source.part||s.target==format!("{}.s{}",o.source.part,staff))))).map(|f|&v.start+&(&f.time-written_start));
                until = stop.clone().unwrap_or_else(|| &v.start + &v.length);
                let opening_end = &o.visit_start + &o.visit_length;
                if attacks.iter().any(|a| {
                    a.part == o.source.part
                        && a.staff == staff
                        && a.onset >= opening_end
                        && a.onset < until
                        && a.onset >= v.start
                }) {
                    return Err(format!(
                        "practice.boundary_uncovered_fresh_attack: {} staff {} beyond opening visit before next authored legacy control",
                        o.source.part, staff
                    ));
                }
                if stop.is_some() {
                    break;
                }
                previous_end = Some(written_start + len);
            }
            receipts.push(BoundaryCoverageReceipt {
                part: o.source.part.clone(),
                staff,
                opening_at: b.at.clone(),
                opening_end: &o.visit_start + &o.visit_length,
                until_legacy_or_end: until,
                post_window_fresh_attacks: 0,
            });
        }
    }
    Ok(receipts)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(reset: bool) -> String {
        format!(
            "!score 1.1 unit=q\n!requires scorestream.instrument@1\n!requires scorestream.fermata@1\n!requires scorestream.articulation@1\n!requires scorestream.authored_dynamics@1\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m1 len=1\n+ p scorestream.instrument midi_ref=\"i\" name=\"Violin\" pan=0 program=41 source_id=\"i\" volume=100\n= p sound_dynamics=4389/50\nv C4:1 id=a\n+ #a scorestream.articulation.event event_index=0\n+ #a scorestream.articulation.notations event_index=0 wrapper_index=0\n+ #a scorestream.articulation.group event_index=0 group_index=0 wrapper_index=0\n+ #a scorestream.articulation.item event_index=0 group_index=0 item_index=0 sign=staccato wrapper_index=0\n+ #m1 scorestream.authored_dynamics part=p measure=m1 index=0 order=0 cursor=1 visual_at=2/3 sounding_at=1 sign=\"p\" percent=1111/25 offset=-1/3\n.\n@1\n| p m2 len=1\nv D4:1 id=b\n+ #m2 scorestream.authored_dynamics part=p measure=m2 index=1 order=0 cursor=0 visual_at=1 sounding_at=1 sign=\"ff\" percent=10667/100\n.\n@2\n| p m3 len=1\n{}v E4:1 id=c\n+ #c scorestream.fermata\n.\n!end at=3\n",
            if reset {
                "= p sound_dynamics=1111/25\n"
            } else {
                ""
            }
        )
    }
    fn run(source: &str) -> Result<TechniquePlaybackPlan, String> {
        plan_source_with_boundary_practice_joint_policies_and_pizzicato(
            source,
            PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,
            PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
            PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
            PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
            PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
            &[PracticeMidiSelection {
                part: "p".into(),
                instrument: "violin".into(),
                bank: 0,
                program_zero_based: 40,
            }],
            ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
            &ExplicitGracePolicy { groups: vec![] },
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &scorestream_codec::fermata_timing::ExplicitSynchronizedFermataPolicy {
                groups: vec![
                    scorestream_codec::fermata_timing::ExplicitSynchronizedHoldGroup {
                        boundary: "3".parse().unwrap(),
                        hold: scorestream_codec::fermata_timing::ExplicitHoldGroup {
                            id: "hold".into(),
                            members: vec![EventId("occ2:c".into())],
                            added_seconds: "1/4".parse().unwrap(),
                        },
                    },
                ],
            },
            FermataSoundPolicy::SustainSelectedAnchors,
            PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
        )
    }
    #[test]
    fn sealed_practice_compiler_changes_only_witnessed_new_attack_and_keeps_strict_blocked() {
        let s = source(true);
        assert!(
            plan_source(&s)
                .unwrap_err()
                .contains("authored_dynamics.performance_required")
        );
        let q = run(&s).unwrap();
        let receipt = q.boundary_dynamics_receipt().unwrap();
        assert_eq!(receipt.attacks.len(), 1);
        let a = &receipt.attacks[0];
        assert_eq!(a.written_id, "b");
        assert_eq!(a.exact_note_on.to_string(), "96003/1000");
        assert_eq!(a.integer_velocity, 96);
        assert_eq!(receipt.schedule.written_statements.len(), 2);
        assert_eq!(receipt.coverage[0].until_legacy_or_end.to_string(), "2");
        assert_eq!(
            q.source_sha256(),
            format!("{:x}", Sha256::digest(s.as_bytes()))
        );
        let before = q
            .plan()
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("a"))
            .unwrap();
        let after = q
            .plan()
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("c"))
            .unwrap();
        assert_eq!(before.velocity, 79);
        assert_eq!(after.velocity, 40);
        assert_eq!(
            a.original_envelope
                .iter()
                .map(|p| p.offset_seconds)
                .collect::<Vec<_>>(),
            a.realized_envelope
                .iter()
                .map(|p| p.offset_seconds)
                .collect::<Vec<_>>()
        );
        assert!(plan_source(&s).is_err());
    }
    #[test]
    fn already_sounding_tied_voice_is_not_retargeted() {
        let s = source(true)
            .replace(
                "!voice v part=p staff=1\n",
                "!voice v part=p staff=1\n!voice w part=p staff=1\n",
            )
            .replace(
                "v C4:1 id=a\n",
                "v C4:1 id=a\nw G4:1 id=held\n~ tie held_tie start=#held\n",
            )
            .replace(
                "v D4:1 id=b\n",
                "v D4:1 id=b\nw G4:1 id=held_tail\n~ tie held_tie end=#held_tail\n",
            );
        let q = run(&s).unwrap();
        let receipt = q.boundary_dynamics_receipt().unwrap();
        assert_eq!(receipt.attacks.len(), 1);
        assert_eq!(receipt.attacks[0].written_id, "b");
        let held = q
            .plan()
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some("held"))
            .unwrap();
        assert_eq!(held.velocity, 79);
        assert_eq!(held.start_seconds, 0.);
        assert!(
            !q.plan()
                .notes
                .iter()
                .any(|n| n.written_id.as_deref() == Some("held_tail"))
        );
    }
    #[test]
    fn no_silent_drop_of_fresh_attacks_after_the_opening_window() {
        assert!(
            run(&source(false))
                .unwrap_err()
                .contains("practice.boundary_uncovered_fresh_attack")
        );
    }
    #[test]
    fn isolated_lifted_control_needs_separate_semantics() {
        let s=source(true).replace(".\n@2\n", "+ #m2 scorestream.authored_dynamics part=p measure=m2 index=2 order=1 cursor=1/2 visual_at=3/2 sounding_at=3/2 sign=\"f\" percent=4389/50\n.\n@2\n");
        assert!(run(&s).unwrap_err().contains("practice.boundary_coverage"));
    }
}
