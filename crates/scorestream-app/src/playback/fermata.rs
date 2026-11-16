//! Deliberately narrow sound realization for an explicitly qualified source.
use super::{
    PlaybackPlan, RealizationEffect, compile_performed, continuity::Continuity,
    diagnostic::Diagnostics, format_diagnostics,
};
use scorestream_codec::{
    Pitch, Rational,
    fermata_timing::{ExplicitFermataPolicy, SeekPosition, TimingLimits, TimingProjection},
    notation::*,
};
use std::collections::{BTreeMap, BTreeSet};

/// No default: callers must explicitly choose this strict initial sound profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FermataSoundPolicy {
    /// Selected terminal notes/tie tails sustain to their exact written endpoint
    /// and through the hold, overriding the ordinary 23/25 gate. Selected interior
    /// tie segments retain one chain with its ordinary tail gate. Unselected
    /// audible chains crossing/ending at a hold reject. No breath/envelope guess.
    SustainSelectedAnchors,
}

#[derive(Clone, Debug)]
pub struct ExplicitFermataPlayback {
    pub plan: PlaybackPlan,
    /// Exact hold intervals, source receipts, policy provenance and seek clock.
    /// Existing UI geometry is not hold-aware; callers must use this separately.
    pub timing: TimingProjection,
    pub sound_policy: FermataSoundPolicy,
}
impl ExplicitFermataPlayback {
    pub fn seek(&self, seconds: &Rational) -> SeekPosition {
        self.timing.seek(seconds)
    }
}

fn parse(source: &str) -> Result<scorestream_codec::Document, String> {
    scorestream_codec::parse(source).map_err(format_diagnostics)
}

/// Read-only discovery for explicit policy entry, never a naked playable score.
pub fn discover_source_fermatas(source: &str) -> Result<FermataDiscovery, String> {
    discover_document_fermatas(&parse(source)?).map_err(format_diagnostics)
}

/// Actual source-to-PlaybackPlan integration, with explicit holds and explicit
/// gate/sound policy. The original source is neither rewritten nor stripped.
/// Ordinary `plan_source` deliberately continues to reject authored fermatas.
pub fn plan_source_with_explicit_fermatas(
    source: &str,
    policy: &ExplicitFermataPolicy,
    sound_policy: FermataSoundPolicy,
) -> Result<ExplicitFermataPlayback, String> {
    let doc = parse(source)?;
    let qualified = perform_document_with_explicit_fermatas(
        &doc,
        policy,
        PerformanceLimits::default(),
        TimingLimits::default(),
    )
    .map_err(format_diagnostics)?;
    match sound_policy {
        FermataSoundPolicy::SustainSelectedAnchors => (),
    }
    let performed = qualified.performed();
    let plan = compile_performed(
        &performed.score,
        &doc,
        Some(performed),
        Some(qualified.timing()),
    )?;
    Ok(ExplicitFermataPlayback {
        plan,
        timing: qualified.into_timing(),
        sound_policy,
    })
}

/// Source-qualified explicit latest-release synchronization. Earlier rest markers
/// retain their own written/performed end; the one global hold occurs at the
/// caller's selected boundary. No automatic grouping or seconds are supplied.
pub fn plan_source_with_synchronized_fermatas(
    source: &str,
    policy: &scorestream_codec::fermata_timing::ExplicitSynchronizedFermataPolicy,
    sound_policy: FermataSoundPolicy,
) -> Result<ExplicitFermataPlayback, String> {
    let doc = parse(source)?;
    let qualified = perform_document_with_synchronized_fermatas(
        &doc,
        policy,
        PerformanceLimits::default(),
        TimingLimits::default(),
    )
    .map_err(format_diagnostics)?;
    match sound_policy {
        FermataSoundPolicy::SustainSelectedAnchors => (),
    }
    let performed = qualified.performed();
    let plan = compile_performed(
        &performed.score,
        &doc,
        Some(performed),
        Some(qualified.timing()),
    )?;
    Ok(ExplicitFermataPlayback {
        plan,
        timing: qualified.into_timing(),
        sound_policy,
    })
}

/// Validate every hold against actual audible chains before compiling any plan.
/// Returns only selected terminal segments whose gate is deliberately overridden.
pub(super) fn validate_sound_policy(
    score: &ResolvedScore,
    continuity: &Continuity,
    timing: &TimingProjection,
    articulation_policy: Option<ExplicitArticulationPolicy>,
    endpoint: Option<super::PracticeHoldEndpointPolicy>,
    before_hold_tails: &mut BTreeSet<usize>,
    warnings: &mut Diagnostics,
) -> Result<BTreeSet<usize>, String> {
    let indices: BTreeMap<_, _> = score
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (&e.id, i))
        .collect();
    let selected: BTreeSet<_> = timing.fermatas.iter().map(|f| &f.id).collect();
    let mut root_by_event = BTreeMap::new();
    let mut audible = Vec::new();
    let mut held_tails = BTreeSet::new();
    for (root, event) in score.events.iter().enumerate() {
        if continuity.targets.contains(&root) {
            continue;
        }
        let chain = continuity.chain(root)?;
        // Cue is a silent event role, not a small printed note. Check the whole
        // sounding chain before using its root to decide audibility: otherwise a
        // normal root sounds through a cue tail, or a cue root erases normal
        // continuations. Neither mixed-role interpretation is supported here.
        let has_cue = chain
            .iter()
            .any(|&i| score.events[i].role == EventRole::Cue);
        let has_non_cue = chain
            .iter()
            .any(|&i| score.events[i].role != EventRole::Cue);
        if has_cue && has_non_cue {
            return Err(format!(
                "fermata.mixed_cue_tie: Sounding tie chain {} mixes silent cue and ordinary event roles",
                event.id
            ));
        }
        let tail = *chain.last().expect("chain has root");
        let sounds = event.role != EventRole::Cue
            && continuity.pitch[root].as_ref().is_some_and(|p| {
                let midi = (p + &Rational::from_integer(12)).to_f64();
                midi.is_finite() && (0.0..=127.0).contains(&midi)
            });
        for &index in &chain {
            if sounds {
                root_by_event.insert(&score.events[index].id, root);
            }
        }
        if sounds {
            if selected.contains(&score.events[tail].id) {
                held_tails.insert(tail);
            }
            audible.push((root, tail));
        }
    }
    for f in &timing.fermatas {
        let event = &score.events[indices[&f.id]];
        if event.role == EventRole::Cue
            || (event.written_pitch != Pitch::Rest && !root_by_event.contains_key(&f.id))
        {
            return Err(format!(
                "fermata.unsupported_sound_anchor: {} is cue, unpitched/unmapped or outside the audible pitch range",
                f.id
            ));
        }
    }
    let tail_by_root: BTreeMap<_, _> = audible.iter().copied().collect();
    // The original hold profile rejects audible articulations. The separately
    // selected joint policy may realize only simple signs on chains disjoint
    // from every hold; it never chooses precedence on a held/tied chain.
    for d in &score.directions {
        match (&d.kind, &d.scope) {
            (DirectionKind::Articulation { value }, Scope::Event(id))
                if root_by_event.contains_key(id) =>
            {
                if articulation_policy == Some(ExplicitArticulationPolicy::SimpleStaccatoAccentV1)
                    && matches!(value, Articulation::Staccato | Articulation::Accent)
                {
                    let index = indices[id];
                    if (*value == Articulation::Staccato && continuity.ties.contains_key(&index))
                        || (*value == Articulation::Accent && continuity.targets.contains(&index))
                    {
                        return Err("articulation.tie_interpretation: Joint policy cannot discard an interior staccato or invent a tied-continuation attack".into());
                    }
                    let root = root_by_event[id];
                    let tail = tail_by_root.get(&root).expect("audible chain indexed");
                    let start = &score.events[root].start;
                    let end = score.events[*tail].end();
                    let next = timing.holds().partition_point(|h| &h.boundary <= start);
                    if timing.holds().get(next).is_some_and(|h| h.boundary <= end) {
                        return Err(format!(
                            "articulation.hold_interpretation: Articulated chain {id} intersects a selected hold; gate/attack precedence requires another policy"
                        ));
                    }
                    continue;
                }
                return Err(format!(
                    "fermata.articulation_unsupported: Audible articulation on {id} needs a separate hold sound policy"
                ));
            }
            _ => (),
        }
    }
    let roots_at_holds: Vec<BTreeSet<_>> = timing
        .holds()
        .iter()
        .map(|h| {
            h.members
                .iter()
                .filter_map(|id| root_by_event.get(id).copied())
                .collect()
        })
        .collect();
    for (root, tail) in audible {
        let start = &score.events[root].start;
        let end = score.events[tail].end();
        let lo = timing.holds().partition_point(|h| &h.boundary <= start);
        let hi = timing.holds().partition_point(|h| h.boundary <= end);
        for (offset, roots) in roots_at_holds[lo..hi].iter().enumerate() {
            if !roots.contains(&root) {
                let hold=&timing.holds()[lo+offset];
                if endpoint.is_some() && end==hold.boundary {
                    before_hold_tails.insert(tail);
                    warnings.add("practice.unmarked_endpoint_before_hold","fermata.endpoint",RealizationEffect::Policy,&Origin {object_id:score.events[root].id.0.clone(),source:score.events[root].origin.source},&format!("Unmarked audible chain {} ends exactly at hold {} quarter {}; explicit rehearsal releases its terminal default gate at exact pre-hold seconds {} (after-hold seconds {}), with no extra sustain or reattack. Subsequent onsets retain after-hold time.",score.events[root].id,hold.id,hold.boundary,hold.seconds_start,hold.seconds_end));
                    continue;
                }
                return Err(format!(
                    "fermata.unselected_overlap: Audible chain {} crosses or ends at hold {} without a selected anchor in that group",
                    score.events[root].id,
                    timing.holds()[lo + offset].id
                ));
            }
        }
    }
    for hold in timing.holds() {
        if hold.synchronization == scorestream_codec::fermata_timing::HoldSynchronization::ExplicitLatestReleaseWithEarlierRests {
            warnings.add("fermata.explicit_synchronization", "fermata.synchronization", RealizationEffect::Policy,
                &Origin { object_id: hold.id.clone(), source: score.events[indices[&hold.members[0]]].origin.source },
                &format!("Explicit editorial synchronization at performed quarter {}: earlier rest anchors keep their original marker intervals; one global hold at the selected latest release.", hold.boundary));
        }
        warnings.add("fermata.explicit_policy_hold", "fermata.hold", RealizationEffect::Policy,
            &Origin { object_id: hold.id.clone(), source: score.events[indices[&hold.members[0]]].origin.source },
            &format!("Added {} seconds from explicit interpretation policy, not from authored sign; strict sustain-selected-anchor sound policy.", hold.added_seconds));
    }
    for &tail in &held_tails {
        warnings.add("fermata.terminal_gate_override", "fermata.sustain", RealizationEffect::Policy,
            &score.events[tail].origin,
            "Explicit sustain-selected-anchor policy replaces the ordinary terminal gate with the exact written endpoint and includes its hold; ties remain one attack.");
    }
    Ok(held_tails)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorestream_codec::fermata_timing::{ExplicitHoldGroup, TimingDiagnostic};

    fn source(event: &str, controls: &str) -> String {
        format!(
            "!score 1.1 unit=q\n!requires scorestream.fermata@1\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv {event} id=a\n+ #a scorestream.fermata shape=normal orientation=upright\n{controls}.\n!end at=1\n"
        )
    }
    fn policy(s: &str) -> ExplicitFermataPolicy {
        ExplicitFermataPolicy {
            groups: vec![ExplicitHoldGroup {
                id: "selected".into(),
                members: discover_source_fermatas(s)
                    .unwrap()
                    .occurrences
                    .into_iter()
                    .map(|o| o.id)
                    .collect(),
                added_seconds: Rational::one(),
            }],
        }
    }
    #[test]
    fn selected_cue_and_unmapped_note_fail_at_audio_boundary() {
        for event in ["C4:1 kind=cue", "u.snare:1"] {
            let s = source(event, "");
            let e = plan_source_with_explicit_fermatas(
                &s,
                &policy(&s),
                FermataSoundPolicy::SustainSelectedAnchors,
            )
            .unwrap_err();
            assert!(e.contains("fermata.unsupported_sound_anchor"), "{e}");
        }
    }
    #[test]
    fn all_audible_articulations_require_a_separate_sound_policy() {
        for value in [
            "staccato",
            "staccatissimo",
            "breath-mark",
            "accent",
            "strong-accent",
            "tenuto",
        ] {
            let s = source("C4:1", &format!("+ #a articulation value={value}\n"));
            let e = plan_source_with_explicit_fermatas(
                &s,
                &policy(&s),
                FermataSoundPolicy::SustainSelectedAnchors,
            )
            .unwrap_err();
            assert!(e.contains("fermata.articulation_unsupported"), "{e}");
        }
    }
    #[test]
    fn default_tempo_and_explicit_gate_policy_are_reported() {
        let s = source("C4:1", "");
        let p = plan_source_with_explicit_fermatas(
            &s,
            &policy(&s),
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .unwrap();
        assert!(p.timing.diagnostics.iter().any(|d| matches!(d, TimingDiagnostic::PerformedDefaultQuarterTempo { from_quarter, until_quarter, .. } if from_quarter == &Rational::zero() && until_quarter == &Rational::one())));
        assert!(
            p.plan
                .diagnostics
                .iter()
                .any(|d| d.code == "fermata.terminal_gate_override")
        );
        assert_eq!(p.plan.notes[0].duration_seconds, 1.5);
    }
    #[test]
    fn missing_ids_fail_for_direct_documents_while_parser_assigned_ids_are_retained() {
        let s = source("C4:1", "")
            .replace("staves=1", "staves=2")
            .replace("@0", "!voice other part=p staff=2\n@0")
            .replace("v C4:1 id=a", "v C4:1 id=a\nother E4:1");
        // Text parsing assigns stable IDs. Discovery retains them and does not
        // panic; the unmarked simultaneous sound then correctly fails policy.
        let mut doc = scorestream_codec::parse(&s).unwrap();
        assert!(doc.events().all(|(_, e)| e.id().is_some()));
        assert_eq!(discover_source_fermatas(&s).unwrap().occurrences.len(), 1);
        assert!(
            plan_source_with_explicit_fermatas(
                &s,
                &policy(&s),
                FermataSoundPolicy::SustainSelectedAnchors
            )
            .unwrap_err()
            .contains("fermata.unselected_overlap")
        );
        // Direct core callers may supply a validated AST without codec-assigned
        // IDs. Every missing source ID returns a structured error before mapping.
        for frame in &mut doc.frames {
            for record in &mut frame.records {
                match record {
                    scorestream_codec::Record::Event(e) if e.voice == "other" => {
                        e.attributes.remove("id");
                    }
                    _ => (),
                }
            }
        }
        for errors in [
            discover_document_fermatas(&doc).unwrap_err(),
            perform_document_with_explicit_fermatas(
                &doc,
                &policy(&s),
                PerformanceLimits::default(),
                TimingLimits::default(),
            )
            .unwrap_err(),
        ] {
            assert!(
                errors.iter().any(|d| d.code == "fermata.source_identity"),
                "{errors:?}"
            );
        }
    }
    #[test]
    fn source_numeric_control_budget_precedes_route_allocation() {
        let s = source("C4:1", "= score tempo=q:1000000000\n");
        let doc = scorestream_codec::parse(&s).unwrap();
        let errors = perform_document_with_explicit_fermatas(
            &doc,
            &ExplicitFermataPolicy { groups: vec![] },
            PerformanceLimits {
                maximum_events: 0,
                ..Default::default()
            },
            TimingLimits {
                maximum_rational_bits: 8,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|d| d.code == "fermata.rational_budget"),
            "{errors:?}"
        );
    }
    fn repeated_rational_source(first: &Rational, marked: bool) -> String {
        let tail = &Rational::one() - first;
        let capability = if marked {
            "!requires scorestream.fermata@1\n"
        } else {
            ""
        };
        let sign = if marked {
            "+ #a scorestream.fermata shape=normal\n"
        } else {
            ""
        };
        format!(
            "!score 1.1 unit=q\n!requires scorestream.navigation@1\n{capability}!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p first len={first}\n+ p scorestream.barline measure=first side=left repeat=forward\nv r:{first} id=a\n{sign}.\n@{first}\n+ p scorestream.barline measure=first side=right repeat=backward\n| p tail len={tail}\nv r:{tail} id=b\n.\n!end at=1\n"
        )
    }
    #[test]
    fn qualified_route_rejects_cumulative_growth_before_policy_interpretation() {
        // Every source time fits eight bits; the second visit ends at 500/251.
        let s = repeated_rational_source(&"250/251".parse().unwrap(), true);
        let doc = scorestream_codec::parse(&s).unwrap();
        let errors = perform_document_with_explicit_fermatas(
            &doc,
            &ExplicitFermataPolicy { groups: vec![] },
            PerformanceLimits::default(),
            TimingLimits {
                maximum_rational_bits: 8,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|d| d.code == "fermata.rational_budget"
                && d.message.contains("before occurrence allocation")),
            "{errors:?}"
        );
    }
    #[test]
    fn qualified_route_bounds_shifted_controls_even_with_small_measure_endpoints() {
        let s = repeated_rational_source(&"1/2".parse().unwrap(), true)
            .replace("@1/2\n", "@1/251\n= score tempo=q:120\n.\n@1/2\n");
        let doc = scorestream_codec::parse(&s).unwrap();
        // Both repeated measure endpoints are small, but the second visit's
        // mid-measure tempo lands at 253/502, beyond the eight-bit denominator.
        let errors = perform_document_with_explicit_fermatas(
            &doc,
            &ExplicitFermataPolicy { groups: vec![] },
            PerformanceLimits::default(),
            TimingLimits {
                maximum_rational_bits: 8,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|d| d.code == "fermata.rational_budget"
                && d.message.contains("before occurrence allocation")),
            "{errors:?}"
        );
    }
    #[test]
    fn discovery_bounds_repeated_times_without_calling_the_policy_clock() {
        // Odd 4096-bit denominator, with both written measures ending within 1.
        // Repeating the first measure needs a 4097-bit performed numerator.
        let mut denominator = Rational::one();
        for _ in 0..4096 {
            denominator = &denominator * &Rational::from_integer(2);
        }
        denominator = &denominator - &Rational::one();
        let first = &(&denominator - &Rational::one()) / &denominator;
        let s = repeated_rational_source(&first, true);
        let error = discover_source_fermatas(&s).unwrap_err();
        assert!(
            error.contains("fermata.rational_budget")
                && error.contains("before occurrence allocation"),
            "{error}"
        );
        // Separate ordinary navigation fixture retains its legacy unbounded
        // exact-quarter arithmetic; no fermata semantics are bypassed in playback.
        let ordinary = scorestream_codec::parse(&repeated_rational_source(&first, false)).unwrap();
        assert!(perform_document(&ordinary, PerformanceLimits::default()).is_ok());
    }
    // Direct SFS is intentional: the current common MusicXML profile rejects
    // cue-role import before this source-to-playback sound policy can run.
    fn cue_tie_source(cue: Option<usize>, anchor: Option<&str>) -> String {
        let mut s = "!score 1.1 unit=q\n".to_owned();
        if anchor.is_some() {
            s += "!requires scorestream.fermata@1\n";
        }
        s += "!part p instrument=piano staves=1\n!voice v part=p staff=1\n";
        for (i, (id, phase)) in [("a", "start"), ("b", "continue"), ("c", "end")]
            .into_iter()
            .enumerate()
        {
            s += &format!("@{i}\n");
            if i == 0 {
                s += "| p m len=4\n";
            }
            let role = if cue == Some(i) { " kind=cue" } else { "" };
            s += &format!("v G4:1 id={id}{role}\n~ tie chain {phase}=#{id}\n");
            if anchor == Some(id) {
                s += &format!("+ #{id} scorestream.fermata shape=normal\n");
            }
            s += ".\n";
        }
        s += "@3\nv r:1 id=pause\n";
        if anchor == Some("pause") {
            s += "+ #pause scorestream.fermata shape=normal\n";
        }
        s += ".\n!end at=4\n";
        s
    }
    #[test]
    fn qualified_sounding_ties_reject_cue_tail_interior_and_root_roles() {
        for (cue, anchor) in [(2, "a"), (1, "a"), (0, "pause")] {
            let s = cue_tie_source(Some(cue), Some(anchor));
            let error = plan_source_with_explicit_fermatas(
                &s,
                &policy(&s),
                FermataSoundPolicy::SustainSelectedAnchors,
            )
            .unwrap_err();
            assert!(
                error.contains("fermata.mixed_cue_tie"),
                "cue segment {cue}: {error}"
            );
        }
        // All-normal ties still sustain the selected interior segment with one
        // attack, and a visually small note does not acquire a silent cue role.
        for s in [
            cue_tie_source(None, Some("a")),
            cue_tie_source(None, Some("a")).replace("id=b", "id=b w=4 note_size=cue"),
        ] {
            let result = plan_source_with_explicit_fermatas(
                &s,
                &policy(&s),
                FermataSoundPolicy::SustainSelectedAnchors,
            )
            .unwrap();
            assert_eq!(result.plan.notes.len(), 1);
            assert!((result.plan.notes[0].duration_seconds - 2.46).abs() < 1e-9);
        }
    }
    #[test]
    fn ordinary_cue_tie_behavior_is_unchanged_by_qualified_policy() {
        // Independent no-fermata fixtures pin the existing ordinary semantics.
        for cue in [None, Some(1), Some(2)] {
            let plan = crate::playback::plan_source(&cue_tie_source(cue, None)).unwrap();
            assert_eq!(plan.notes.len(), 1);
            assert!((plan.notes[0].duration_seconds - 1.46).abs() < 1e-9);
        }
        assert!(
            crate::playback::plan_source(&cue_tie_source(Some(0), None))
                .unwrap()
                .notes
                .is_empty()
        );
        // A wholly silent, unselected all-cue chain stays silent even when an
        // unrelated later rest has a fully explicit hold.
        let all_cue = cue_tie_source(Some(0), Some("pause"))
            .replace("id=b", "id=b kind=cue")
            .replace("id=c", "id=c kind=cue");
        let result = plan_source_with_explicit_fermatas(
            &all_cue,
            &policy(&all_cue),
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .unwrap();
        assert!(result.plan.notes.is_empty());
        assert_eq!(result.plan.duration_seconds, 3.0);
    }
}
