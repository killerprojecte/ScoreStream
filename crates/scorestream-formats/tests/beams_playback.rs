//! Finite synthetic MusicXML -> shared model -> SFS -> actual playback contracts.
//! Printed beams must not change repeat, explicit-grace, explicit-fermata, or
//! joint audio. These fixtures are not transcribed/repaired Beethoven sources.
use scorestream_app::playback::{
    FermataSoundPolicy, GraceSoundPolicy, PlaybackPlan, discover_source_fermatas,
    discover_source_graces, discover_source_graces_and_fermatas, plan_source,
    plan_source_with_explicit_fermatas, plan_source_with_explicit_graces,
    plan_source_with_explicit_graces_and_fermatas,
};
use scorestream_core::{
    Rational, SourceRange,
    fermata_timing::{ExplicitFermataPolicy, ExplicitHoldGroup, FermataOccurrence},
    notation::{
        EventId, ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation, GraceDiscovery,
        GraceQuarterAllocation,
    },
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
use std::collections::{BTreeMap, BTreeSet};

const BEGIN: &str = "<beam>begin</beam>";
const END: &str = "<beam number=\"1\">end</beam>";
const GRACE_BEGIN: &str = "<beam>begin</beam><beam number=\"2\">begin</beam>";
const GRACE_END: &str = "<beam number=\"1\">end</beam><beam number=\"2\">end</beam>";
const GRACE_BEAMS: &[(&str, &str)] = &[("z-grace", GRACE_BEGIN), ("a-grace", GRACE_END)];

fn r(value: &str) -> Rational {
    value.parse().unwrap()
}

fn note_range(xml: &str, id: &str) -> std::ops::Range<usize> {
    let start = xml.find(&format!("<note id=\"{id}\">")).unwrap();
    let end = start + xml[start..].find("</note>").unwrap() + "</note>".len();
    start..end
}

fn sources(xml: &str, annotations: &[(&str, &str)], capabilities: &[&str]) -> [String; 2] {
    let mut beamed_xml = xml.to_owned();
    for (id, annotation) in annotations {
        let range = note_range(&beamed_xml, id);
        beamed_xml.insert_str(range.end - "</note>".len(), annotation);
    }
    let plain = MusicXml.read(xml).unwrap().value;
    let beamed = MusicXml.read(&beamed_xml).unwrap().value;
    assert!(plain.beams.is_empty());
    assert_eq!(
        beamed.beams.len(),
        annotations
            .iter()
            .map(|(_, a)| a.matches("<beam").count())
            .sum::<usize>()
    );
    // Comparing complete common models proves that annotation was the only edit:
    // written voices, event order, pitches, timing, controls and grace/fermata
    // semantics were neither repaired nor stripped to obtain playable input.
    let mut stripped = beamed.clone();
    stripped.beams.clear();
    assert_eq!(plain, stripped);
    let pair = [plain, beamed].map(|model| {
        let wire = SfsAdapter.write(&model).unwrap().value;
        // SFS already orders independent voices by onset rather than MusicXML
        // backup traversal. Normalize only event-container order for this model
        // roundtrip; beam order and every playback comparison stay untouched.
        let comparable = |mut score: scorestream_core::score::ScoreDocument| {
            for part in &mut score.parts {
                for measure in &mut part.measures {
                    measure.events.sort_by(|a, b| {
                        (&a.offset, &a.voice, &a.id).cmp(&(&b.offset, &b.voice, &b.id))
                    });
                }
            }
            score
        };
        assert_eq!(
            comparable(SfsAdapter.read(&wire).unwrap().value),
            comparable(model)
        );
        wire
    });
    let required = |source: &str| {
        scorestream_codec::parse(source)
            .unwrap()
            .required_extensions
            .into_iter()
            .collect::<BTreeSet<_>>()
    };
    let expected: BTreeSet<_> = capabilities.iter().map(|c| (*c).to_owned()).collect();
    assert_eq!(required(&pair[0]), expected);
    let mut with_beam = expected;
    with_beam.insert("scorestream.beam@1".into());
    assert_eq!(required(&pair[1]), with_beam);
    assert_ne!(pair[0], pair[1]);
    pair
}

fn same_audio(before: &PlaybackPlan, after: &PlaybackPlan) {
    // Source spans move when required capabilities and beam records are added.
    // All actual audio, written/performed identities, routes, voices, expression
    // envelopes, markers, measure clocks and diagnostic semantics must match.
    let without_source_spans = |plan: &PlaybackPlan| {
        let mut plan = plan.clone();
        for diagnostic in &mut plan.diagnostics {
            diagnostic.source = SourceRange::default();
        }
        plan
    };
    assert_eq!(without_source_spans(before), without_source_spans(after));
}

fn unknown_capability(source: &str) -> String {
    let requirement = "!requires scorestream.beam@1";
    assert!(source.contains(requirement));
    source.replace(
        requirement,
        &format!("{requirement}\n!requires scorestream.unknown-beam-test@1"),
    )
}

fn grace_policy(discovery: &GraceDiscovery) -> ExplicitGracePolicy {
    let mut groups = BTreeMap::<EventId, Vec<GraceQuarterAllocation>>::new();
    for occurrence in &discovery.occurrences {
        let quarters = match occurrence.written_id.0.as_str() {
            "z-grace" => r("1/8"),
            "a-grace" => r("1/4"),
            id => panic!("unexpected synthetic grace {id}"),
        };
        groups
            .entry(occurrence.principal.clone())
            .or_default()
            .push(GraceQuarterAllocation {
                event: occurrence.id.clone(),
                quarters,
            });
    }
    assert!(!groups.is_empty());
    ExplicitGracePolicy {
        groups: groups
            .into_iter()
            .map(|(principal, mut members)| {
                // Policy order must not replace authored grace order.
                members.reverse();
                ExplicitGraceGroup {
                    principal,
                    allocation: GraceAllocation::ExplicitFollowingQuarters { members },
                }
            })
            .collect(),
    }
}

fn hold_policy(occurrences: &[FermataOccurrence]) -> ExplicitFermataPolicy {
    assert!(!occurrences.is_empty());
    assert!(occurrences.iter().all(|o| o.quarter_end == r("1")));
    ExplicitFermataPolicy {
        groups: vec![ExplicitHoldGroup {
            id: "synthetic-explicit-hold".into(),
            members: occurrences.iter().map(|o| o.id.clone()).collect(),
            added_seconds: r("1"),
        }],
    }
}

#[test]
fn beamed_repeat_pairs_preserve_route_tempo_expression_and_occurrence_identity() {
    // Retain the existing synthetic repeat/ending/transpose/control structure,
    // replacing each quarter with two eligible eighths in BOTH source variants.
    let mut xml = include_str!("fixtures/navigation-performed.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>");
    for id in ["before", "a", "b", "c", "d"] {
        let range = note_range(&xml, id);
        let first = xml[range.clone()].replace("<type>quarter</type>", "<type>eighth</type>");
        let second = first.replace(&format!("id=\"{id}\""), &format!("id=\"{id}-tail\""));
        xml.replace_range(range, &(first + &second));
    }
    let [plain, beamed] = sources(
        &xml,
        &[
            ("before", BEGIN),
            ("before-tail", END),
            ("a", BEGIN),
            ("a-tail", END),
            ("b", BEGIN),
            ("b-tail", END),
            ("c", BEGIN),
            ("c-tail", END),
            ("d", BEGIN),
            ("d-tail", END),
        ],
        &["scorestream.instrument@1", "scorestream.navigation@1"],
    );
    let before = plan_source(&plain).unwrap();
    let after = plan_source(&beamed).unwrap();
    same_audio(&before, &after);
    assert_eq!(after.duration_seconds, 5.5);
    assert_eq!(
        after
            .notes
            .iter()
            .map(|n| n.written_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "before",
            "before-tail",
            "a",
            "a-tail",
            "b",
            "b-tail",
            "a",
            "a-tail",
            "c",
            "c-tail",
            "d",
            "d-tail"
        ]
    );
    assert_eq!(
        after.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [45, 45, 45, 45, 90, 90, 45, 45, 45, 45, 45, 45]
    );
    assert_eq!(
        after
            .notes
            .iter()
            .map(|n| n.start_seconds)
            .collect::<Vec<_>>(),
        [0., 0.5, 1., 1.5, 2., 2.25, 2.5, 3., 3.5, 4., 4.5, 5.]
    );
    assert_ne!(after.notes[2].id, after.notes[6].id);
    assert!(plan_source(&unknown_capability(&beamed)).is_err());
}

#[test]
fn beamed_grace_pair_preserves_explicit_windows_and_keeps_ordinary_playback_closed() {
    let [plain, beamed] = sources(
        include_str!("fixtures/grace-playback.musicxml"),
        GRACE_BEAMS,
        &["scorestream.instrument@1", "scorestream.grace@1"],
    );
    // Discover and choose policies separately for each unchanged source, rather
    // than substituting a score, stripping capabilities, or comparing its hash.
    let before_discovery = discover_source_graces(&plain).unwrap();
    let after_discovery = discover_source_graces(&beamed).unwrap();
    assert_eq!(before_discovery, after_discovery);
    let before = plan_source_with_explicit_graces(
        &plain,
        &grace_policy(&before_discovery),
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    )
    .unwrap();
    let policy = grace_policy(&after_discovery);
    let after = plan_source_with_explicit_graces(
        &beamed,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    )
    .unwrap();
    same_audio(before.plan(), after.plan());
    assert_eq!(before.schedule(), after.schedule());
    assert_eq!(after.plan().notes.len(), 3);
    assert_eq!(after.plan().notes[0].midi_note, 74.);
    assert_eq!(after.plan().notes[1].midi_note, 76.);
    assert_eq!(after.schedule().windows()[0].seconds_end, r("5/16"));
    assert!(
        after.plan().notes[1]
            .gain_envelope
            .windows(2)
            .any(|w| w[0].gain != w[1].gain)
    );
    assert!(
        plan_source(&beamed)
            .unwrap_err()
            .contains("grace.performance_unsupported")
    );
    let unknown = unknown_capability(&beamed);
    assert!(discover_source_graces(&unknown).is_err());
    assert!(
        plan_source_with_explicit_graces(
            &unknown,
            &policy,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
        )
        .is_err()
    );
}

fn fermata_xml() -> String {
    // Two independent voices, common-release note/rest fermatas, mid-note tempo
    // and dynamics, plus an ordinary beamed pair after the explicit global hold.
    let note = |id: &str, voice: u8, pitch: &str, ticks: u8, held: bool| {
        let body = if pitch == "rest" {
            "<rest/>".into()
        } else {
            format!("<pitch><step>{pitch}</step><octave>4</octave></pitch>")
        };
        let kind = if ticks == 4 { "eighth" } else { "quarter" };
        let notation = if held {
            "<notations><fermata/></notations>"
        } else {
            ""
        };
        format!(
            "<note id=\"{id}\">{body}<duration>{ticks}</duration><voice>{voice}</voice><type>{kind}</type>{notation}</note>"
        )
    };
    let notes = [
        note("lead", 1, "C", 4, false),
        note("held", 1, "G", 4, true),
        note("tail", 1, "D", 8, false),
        "<backup><duration>16</duration></backup>".into(),
        note("lead-rest", 2, "rest", 4, false),
        note("held-rest", 2, "rest", 4, true),
        note("other-a", 2, "E", 4, false),
        note("other-b", 2, "F", 4, false),
    ]
    .concat();
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>8</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><sound tempo="120" dynamics="50"/><sound tempo="60" dynamics="100"><offset>6</offset></sound>{notes}</measure></part></score-partwise>"#
    )
}

#[test]
fn beamed_note_and_rest_fermatas_preserve_hold_clock_expression_and_both_voices() {
    let [plain, beamed] = sources(
        &fermata_xml(),
        &[
            ("lead", BEGIN),
            ("held", END),
            ("lead-rest", BEGIN),
            ("held-rest", END),
            ("other-a", BEGIN),
            ("other-b", END),
        ],
        &["scorestream.instrument@1", "scorestream.fermata@1"],
    );
    let before_discovery = discover_source_fermatas(&plain).unwrap();
    let after_discovery = discover_source_fermatas(&beamed).unwrap();
    assert_eq!(before_discovery, after_discovery);
    assert_eq!(after_discovery.occurrences.len(), 2);
    let before = plan_source_with_explicit_fermatas(
        &plain,
        &hold_policy(&before_discovery.occurrences),
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap();
    let policy = hold_policy(&after_discovery.occurrences);
    let after = plan_source_with_explicit_fermatas(
        &beamed,
        &policy,
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap();
    same_audio(&before.plan, &after.plan);
    assert_eq!(before.timing, after.timing);
    assert_eq!(before.sound_policy, after.sound_policy);
    assert_eq!(after.timing.duration_seconds(), &r("21/8"));
    assert_eq!(after.timing.before(&r("1")), r("5/8"));
    assert_eq!(after.timing.after(&r("1")), r("13/8"));
    assert_eq!(after.plan.notes.len(), 5);
    assert_eq!(after.plan.event_positions.len(), 7);
    assert_eq!(
        after
            .plan
            .notes
            .iter()
            .map(|n| &n.voice)
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    let held = after
        .plan
        .notes
        .iter()
        .find(|n| n.written_id.as_deref() == Some("held"))
        .unwrap();
    assert_eq!(held.start_seconds, 0.25);
    assert_eq!(held.duration_seconds, 1.375);
    assert!(
        held.gain_envelope
            .windows(2)
            .any(|w| w[0].gain != w[1].gain)
    );
    assert!(
        plan_source(&beamed)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    let unknown = unknown_capability(&beamed);
    assert!(discover_source_fermatas(&unknown).is_err());
    assert!(
        plan_source_with_explicit_fermatas(
            &unknown,
            &policy,
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .is_err()
    );
}

#[test]
fn beamed_joint_source_preserves_both_explicit_policies_without_widening_entrypoints() {
    let [plain, beamed] = sources(
        include_str!("fixtures/joint-playback.musicxml"),
        GRACE_BEAMS,
        &[
            "scorestream.instrument@1",
            "scorestream.grace@1",
            "scorestream.fermata@1",
        ],
    );
    let before_discovery = discover_source_graces_and_fermatas(&plain).unwrap();
    let after_discovery = discover_source_graces_and_fermatas(&beamed).unwrap();
    assert_eq!(before_discovery, after_discovery);
    let before = plan_source_with_explicit_graces_and_fermatas(
        &plain,
        &grace_policy(&before_discovery.graces),
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &hold_policy(&before_discovery.fermatas.occurrences),
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap();
    let grace = grace_policy(&after_discovery.graces);
    let hold = hold_policy(&after_discovery.fermatas.occurrences);
    let after = plan_source_with_explicit_graces_and_fermatas(
        &beamed,
        &grace,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &hold,
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap();
    same_audio(before.plan(), after.plan());
    assert_eq!(before.timing(), after.timing());
    assert_eq!(before.grace_windows(), after.grace_windows());
    assert_eq!(before.grace_groups(), after.grace_groups());
    assert_eq!(before.grace_discovery(), after.grace_discovery());
    assert_eq!(before.grace_sound_policy(), after.grace_sound_policy());
    assert_eq!(before.fermata_sound_policy(), after.fermata_sound_policy());
    assert_eq!(after.plan().notes.len(), 4);
    assert_eq!(after.timing().duration_seconds(), &r("21/8"));
    assert_eq!(after.grace_windows()[0].seconds_start, r("3/2"));
    assert_eq!(after.grace_windows()[1].seconds_start, r("13/8"));
    assert!(plan_source(&beamed).is_err());
    assert!(discover_source_graces(&beamed).is_err());
    assert!(discover_source_fermatas(&beamed).is_err());
    assert!(
        plan_source_with_explicit_graces(
            &beamed,
            &grace,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
        )
        .is_err()
    );
    assert!(
        plan_source_with_explicit_fermatas(
            &beamed,
            &hold,
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .is_err()
    );
    let unknown = unknown_capability(&beamed);
    assert!(discover_source_graces_and_fermatas(&unknown).is_err());
    assert!(
        plan_source_with_explicit_graces_and_fermatas(
            &unknown,
            &grace,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &hold,
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .is_err()
    );
}
