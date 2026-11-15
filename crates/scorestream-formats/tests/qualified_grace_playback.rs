//! Authored synthetic MusicXML → shared model → unchanged SFS → actual grace audio.
//! These finite contracts are not a cleaned/transcribed Beethoven source, an
//! original-score import acceptance, or a claim about sampled audio quality.
use scorestream_app::playback::{
    ExplicitGracePlayback, FermataSoundPolicy, GraceSoundPolicy, discover_source_fermatas,
    discover_source_graces, plan_source, plan_source_with_explicit_fermatas,
    plan_source_with_explicit_graces,
};
use scorestream_core::{
    Duration, Rational,
    fermata_timing::ExplicitFermataPolicy,
    notation::{
        EventId, ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation, GraceQuarterAllocation,
        GraceTimingLimits, PerformanceLimits, lower_document, perform_document,
        perform_document_with_explicit_graces,
    },
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
use std::collections::{BTreeMap, BTreeSet};

fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn note(id: &str, ticks: usize, pitch: &str, notation: &str) -> String {
    format!(
        "<note id=\"{id}\"><pitch><step>{pitch}</step><octave>4</octave></pitch><duration>{ticks}</duration><voice>1</voice>{notation}</note>"
    )
}
fn grace(id: &str, attrs: &str, pitch: &str) -> String {
    format!(
        "<note id=\"{id}\"><grace{attrs}/><pitch><step>{pitch}</step><octave>5</octave></pitch><voice>1</voice><type>16th</type></note>"
    )
}
fn xml(body: &str, quarters: usize) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>Piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\"><measure id=\"m\" number=\"1\"><attributes><divisions>8</divisions><time><beats>{quarters}</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"
    )
}
fn source(xml: &str) -> String {
    let model = MusicXml.read(xml).unwrap().value;
    let before = model.clone();
    let source = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(model, before);
    assert_eq!(SfsAdapter.read(&source).unwrap().value, model);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value,
        model
    );
    source
}
fn singleton(attrs: &str, quarters: usize, controls: &str) -> String {
    source(&xml(
        &format!(
            "<sound tempo=\"120\"/>{controls}{}{}",
            grace("g", attrs, "D"),
            note("main", quarters * 8, "G", "")
        ),
        quarters,
    ))
}
fn play(source: &str, policy: &ExplicitGracePolicy) -> Result<ExplicitGracePlayback, String> {
    let before = scorestream_codec::parse(source).unwrap();
    let text = source.to_owned();
    let result = plan_source_with_explicit_graces(
        source,
        policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    );
    assert_eq!(source, text);
    assert_eq!(scorestream_codec::parse(source).unwrap(), before);
    result
}
fn explicit(source: &str, allocations: &[(&str, &str)]) -> ExplicitGracePolicy {
    let d = discover_source_graces(source).unwrap();
    let mut groups = BTreeMap::<EventId, Vec<GraceQuarterAllocation>>::new();
    for o in &d.occurrences {
        let amount = allocations
            .iter()
            .find(|(id, _)| *id == o.written_id.0)
            .unwrap()
            .1;
        groups
            .entry(o.principal.clone())
            .or_default()
            .push(GraceQuarterAllocation {
                event: o.id.clone(),
                quarters: r(amount),
            });
    }
    ExplicitGracePolicy {
        groups: groups
            .into_iter()
            .map(|(principal, members)| ExplicitGraceGroup {
                principal,
                allocation: GraceAllocation::ExplicitFollowingQuarters { members },
            })
            .collect(),
    }
}
fn authored(source: &str) -> ExplicitGracePolicy {
    let d = discover_source_graces(source).unwrap();
    ExplicitGracePolicy {
        groups: d
            .occurrences
            .iter()
            .map(|o| o.principal.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|principal| ExplicitGraceGroup {
                principal,
                allocation: GraceAllocation::AuthoredFollowingPercent,
            })
            .collect(),
    }
}
fn reject(source: &str, policy: &ExplicitGracePolicy, diagnostic: &str) {
    let error = play(source, policy).unwrap_err();
    assert!(
        error.contains(diagnostic),
        "expected {diagnostic}, got {error}"
    );
}

#[test]
fn authored_singleton_produces_two_real_attacks_without_changing_written_events() {
    let s = singleton(" slash=\"yes\" steal-time-following=\"25\"", 1, "");
    let p = play(&s, &authored(&s)).unwrap();
    assert_eq!(
        p.sound_policy(),
        GraceSoundPolicy::FullGraceThenGatedRemainder
    );
    assert_eq!(p.plan().notes.len(), 2);
    let (g, n) = (&p.plan().notes[0], &p.plan().notes[1]);
    close(g.start_seconds, 0.);
    close(g.duration_seconds, 1. / 8.);
    close(n.start_seconds, 1. / 8.);
    close(n.duration_seconds, 69. / 200.);
    close(n.start_seconds + n.duration_seconds, 47. / 100.);
    close(p.plan().duration_seconds, 1. / 2.);
    close(g.midi_note, 74.);
    close(n.midi_note, 67.);
    assert_eq!(g.instrument, n.instrument);
    let marker = p
        .plan()
        .event_positions
        .iter()
        .find(|e| e.id == "g")
        .unwrap();
    close(marker.start_seconds, 0.);
    close(marker.end_seconds, 0.);
    let principal = p
        .plan()
        .event_positions
        .iter()
        .find(|e| e.id == "main")
        .unwrap();
    close(principal.start_seconds, 0.);
    close(principal.end_seconds, 1. / 2.);
    close(p.plan().measures[0].end_seconds, 1. / 2.);
    assert!(
        !p.plan()
            .diagnostics
            .iter()
            .any(|d| d.code == "grace_unrealized")
    );
    let doc = scorestream_codec::parse(&s).unwrap();
    assert_eq!(
        doc.events()
            .filter(|(_, e)| e.duration == Duration::Grace)
            .count(),
        1
    );
}

#[test]
fn legacy_and_unqualified_apis_do_not_silently_gain_an_interpretation() {
    let s = singleton("", 1, "");
    let doc = scorestream_codec::parse(&s).unwrap();
    assert!(
        lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "grace.performance_unsupported")
    );
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
    assert!(
        plan_source(&s)
            .unwrap_err()
            .contains("grace.performance_unsupported")
    );
    reject(&s, &ExplicitGracePolicy { groups: vec![] }, "grace.");
    // Only this dedicated legacy contract removes its capability, never a source
    // passed to qualified playback or used as a successful imported-score result.
    let legacy = s
        .lines()
        .filter(|l| !l.contains("scorestream.grace"))
        .collect::<Vec<_>>()
        .join("\n");
    let plan = plan_source(&legacy).unwrap();
    assert_eq!(plan.notes.len(), 1);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "grace_unrealized")
    );
    assert!(discover_source_graces(&legacy).is_err());
    assert!(SfsAdapter.read(&legacy).is_err());
}

#[test]
fn combined_authored_fermata_and_grace_rejects_both_qualified_paths() {
    let s = source(&xml(
        &format!(
            "{}{}",
            grace("g", "", "D"),
            note("main", 8, "G", "<notations><fermata/></notations>")
        ),
        1,
    ));
    assert!(discover_source_graces(&s).is_err());
    assert!(discover_source_fermatas(&s).is_err());
    reject(&s, &ExplicitGracePolicy { groups: vec![] }, "fermata");
    let e = plan_source_with_explicit_fermatas(
        &s,
        &ExplicitFermataPolicy { groups: vec![] },
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap_err();
    assert!(e.contains("grace"), "{e}");
}

#[test]
fn two_explicit_graces_follow_source_order_across_tempo_and_numeric_changes() {
    let s = source(include_str!("fixtures/grace-playback.musicxml"));
    let mut policy = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    if let GraceAllocation::ExplicitFollowingQuarters { members } = &mut policy.groups[0].allocation
    {
        members.reverse();
    }
    let p = play(&s, &policy).unwrap();
    assert_eq!(p.plan().notes.len(), 3);
    let expected = [
        (74., 0.25, 0.0625, 45),
        (76., 0.3125, 0.1875, 45),
        (67., 0.5, 1.035, 90),
    ];
    for (n, (pitch, start, duration, velocity)) in p.plan().notes.iter().zip(expected) {
        close(n.midi_note, pitch);
        close(n.start_seconds, start);
        close(n.duration_seconds, duration);
        assert_eq!(n.velocity, velocity);
    }
    let second = &p.plan().notes[1];
    assert!(
        second
            .gain_envelope
            .windows(2)
            .any(|w| (w[0].offset_seconds - 1. / 16.).abs() < 1e-9
                && w[0].offset_seconds == w[1].offset_seconds
                && w[0].gain == 1.
                && w[1].gain == 2.)
    );
    close(p.plan().duration_seconds, 13. / 8.);
    for id in ["z-grace", "a-grace"] {
        let m = p
            .plan()
            .event_positions
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        close(m.start_seconds, 0.25);
        close(m.end_seconds, 0.25);
    }
    let m = p
        .plan()
        .event_positions
        .iter()
        .find(|e| e.id == "principal")
        .unwrap();
    close(m.start_seconds, 0.25);
    close(m.end_seconds, 13. / 8.);
}

#[test]
fn explicit_steal_leaves_unrelated_notes_and_score_boundaries_unchanged() {
    let s = source(&xml(
        &format!(
            "{}{}{}{}",
            grace("g", "", "D"),
            note("main", 8, "G", ""),
            note("unrelated", 8, "A", ""),
            note("tail", 8, "B", "")
        ),
        3,
    ));
    let p = play(&s, &explicit(&s, &[("g", "1/4")])).unwrap();
    for (n, start) in p.plan().notes[2..].iter().zip([0.5, 1.]) {
        close(n.start_seconds, start);
        close(n.duration_seconds, 0.46);
    }
    close(p.plan().duration_seconds, 1.5);
    close(p.plan().measures[0].end_seconds, 1.5);
}

#[test]
fn variable_effective_tempo_forbids_authored_percent_but_accepts_explicit_quarters() {
    let controls = "<sound tempo=\"60\"><offset>8</offset></sound>";
    let authored_source = singleton(" steal-time-following=\"50\"", 2, controls);
    reject(
        &authored_source,
        &authored(&authored_source),
        "grace.percentage_tempo_ambiguous",
    );
    let explicit_source = singleton("", 2, controls);
    let p = play(
        &explicit_source,
        &explicit(&explicit_source, &[("g", "1/2")]),
    )
    .unwrap();
    close(p.plan().notes[0].duration_seconds, 0.25);
    close(p.plan().notes[1].start_seconds, 0.25);
    close(p.plan().duration_seconds, 1.5);
    let unchanged_tempo = singleton(
        " steal-time-following=\"50\"",
        2,
        "<sound tempo=\"120\"><offset>8</offset></sound>",
    );
    assert!(play(&unchanged_tempo, &authored(&unchanged_tempo)).is_ok());
}

#[test]
fn unsupported_hints_are_preserved_and_never_overridden_by_explicit_policy() {
    for attrs in [
        " steal-time-following=\"0\"",
        " steal-time-following=\"100\"",
        " steal-time-previous=\"25\"",
        " make-time=\"1\"",
        " steal-time-following=\"25\" steal-time-previous=\"0\"",
        " steal-time-following=\"25\" make-time=\"0\"",
    ] {
        let s = singleton(attrs, 1, "");
        reject(&s, &authored(&s), "grace.authored_following_unsupported");
        reject(&s, &explicit(&s, &[("g", "1/4")]), "grace.");
    }
    let s = source(&xml(
        &format!(
            "{}{}{}",
            grace("g1", " steal-time-following=\"25\"", "D"),
            grace("g2", "", "E"),
            note("main", 8, "G", "")
        ),
        1,
    ));
    reject(&s, &authored(&s), "grace.authored_following_unsupported");
    reject(&s, &explicit(&s, &[("g1", "1/8"), ("g2", "1/8")]), "grace.");
    let absent = singleton("", 1, "");
    reject(
        &absent,
        &authored(&absent),
        "grace.authored_following_unsupported",
    );
}

#[test]
fn complete_unique_positive_occurrence_allocations_are_required() {
    let s = source(&xml(
        &format!(
            "{}{}{}",
            grace("g1", "", "D"),
            grace("g2", "", "E"),
            note("main", 8, "G", "")
        ),
        1,
    ));
    for amount in ["0", "-1/8", "3/4", "1"] {
        reject(
            &s,
            &explicit(&s, &[("g1", amount), ("g2", "1/4")]),
            "grace.",
        );
    }
    let good = explicit(&s, &[("g1", "1/8"), ("g2", "1/4")]);
    let mut duplicate_group = good.clone();
    duplicate_group.groups.push(good.groups[0].clone());
    reject(&s, &duplicate_group, "grace.policy_duplicate");
    let mut unknown_group = good.clone();
    unknown_group.groups[0].principal = EventId("unknown-principal".into());
    reject(&s, &unknown_group, "grace.policy_principal");
    for edit in [0, 1, 2] {
        let mut policy = good.clone();
        if let GraceAllocation::ExplicitFollowingQuarters { members } =
            &mut policy.groups[0].allocation
        {
            match edit {
                0 => {
                    members.pop();
                }
                1 => {
                    members.push(members[0].clone());
                }
                _ => {
                    members[0].event = EventId("unknown-grace".into());
                }
            }
        }
        reject(
            &s,
            &policy,
            match edit {
                0 => "grace.policy_missing",
                1 => "grace.policy_duplicate",
                _ => "grace.policy_member",
            },
        );
    }
}

#[test]
fn more_than_ordinary_gate_steal_still_leaves_positive_gated_principal() {
    let s = singleton("", 1, "");
    let p = play(&s, &explicit(&s, &[("g", "99/100")])).unwrap();
    close(p.plan().notes[0].duration_seconds, 0.495);
    close(p.plan().notes[1].start_seconds, 0.495);
    close(p.plan().notes[1].duration_seconds, 0.0046);
    close(p.plan().duration_seconds, 0.5);
}

#[test]
fn transposed_graces_share_the_principal_route_and_graphical_cue_size_is_audible() {
    let original = xml(
        &format!(
            "{}{}",
            grace("g", " slash=\"no\"", "D"),
            note("main", 8, "G", "")
        ),
        1,
    );
    let imported = original
        .replacen(
            "</attributes>",
            "<transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes>",
            1,
        )
        .replacen(
            "<type>16th</type>",
            "<type size=\"grace-cue\">16th</type>",
            1,
        );
    let s = source(&imported);
    let p = play(&s, &explicit(&s, &[("g", "1/4")])).unwrap();
    assert_eq!(p.plan().notes.len(), 2);
    assert_eq!(
        p.plan()
            .notes
            .iter()
            .map(|n| n.midi_note)
            .collect::<Vec<_>>(),
        [72., 65.]
    );
    assert_eq!(p.plan().routes.len(), 1);
    assert!(
        p.plan()
            .notes
            .iter()
            .all(|n| n.part == "p" && n.instrument == "piano")
    );
    let model = SfsAdapter.read(&s).unwrap().value;
    assert_eq!(model.graces[0].slash, Some(false));
    assert_eq!(
        model.graces[0].written.as_ref().unwrap().size,
        Some(scorestream_core::score::NoteSize::GraceCue)
    );
}

#[test]
fn out_of_range_graces_or_principals_reject_instead_of_disappearing() {
    let original = xml(
        &format!("{}{}", grace("g", "", "D"), note("main", 8, "G", "")),
        1,
    );
    for bad in [
        original.replacen(
            "<step>D</step><octave>5</octave>",
            "<step>B</step><octave>9</octave>",
            1,
        ),
        original.replacen(
            "<step>G</step><octave>4</octave>",
            "<step>B</step><octave>9</octave>",
            1,
        ),
    ] {
        let s = source(&bad);
        reject(&s, &explicit(&s, &[("g", "1/4")]), "grace.");
    }
}

#[test]
fn desktop_fixture_is_generated_from_the_actual_qualified_rust_plan() {
    let s = source(include_str!("fixtures/grace-playback.musicxml"));
    let p = play(&s, &explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")])).unwrap();
    assert_eq!(p.plan().notes.len(), 3);
    close(p.plan().duration_seconds, 13. / 8.);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/tests/fixtures/grace-playback.json");
    let actual = serde_json::to_value(p.plan()).unwrap();
    if std::env::var_os("UPDATE_GRACE_FIXTURE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let checked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        checked, actual,
        "Regenerate only from the unchanged synthetic XML fixture with UPDATE_GRACE_FIXTURE=1"
    );
}

#[test]
fn exact_schedule_and_seek_keep_grace_anchor_markers_distinct_from_sound_windows() {
    let s = source(include_str!("fixtures/grace-playback.musicxml"));
    let policy = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let p = play(&s, &policy).unwrap();
    let schedule = p.schedule();
    assert_eq!(
        schedule.sound_policy(),
        GraceSoundPolicy::FullGraceThenGatedRemainder
    );
    assert_eq!(schedule.duration_seconds(), r("13/8"));
    assert_eq!(schedule.quarter_end(), &r("2"));
    assert_eq!(schedule.groups().len(), 1);
    assert_eq!(schedule.groups()[0].allocated_quarters, r("3/8"));
    assert_eq!(schedule.groups()[0].allocation, policy.groups[0].allocation);
    let expected = [
        ("1/2", "5/8", "1/4", "5/16"),
        ("5/8", "7/8", "5/16", "1/2"),
        ("7/8", "191/100", "1/2", "307/200"),
    ];
    for (window, (q0, q1, s0, s1)) in schedule.windows().iter().zip(expected) {
        assert_eq!(window.quarter_start, r(q0));
        assert_eq!(window.quarter_end, r(q1));
        assert_eq!(window.seconds_start, r(s0));
        assert_eq!(window.seconds_end, r(s1));
        assert_eq!(schedule.seconds(&r(q0)).unwrap(), r(s0));
        assert_eq!(schedule.seconds(&r(q1)).unwrap(), r(s1));
        assert_eq!(p.seek(&r(s0)), r(q0));
        assert_eq!(p.seek(&r(s1)), r(q1));
    }
    assert_eq!(
        schedule
            .windows()
            .iter()
            .map(|w| w.role)
            .collect::<Vec<_>>(),
        [
            scorestream_core::notation::EventRole::Grace,
            scorestream_core::notation::EventRole::Grace,
            scorestream_core::notation::EventRole::Note
        ]
    );
    assert_eq!(p.seek(&r("-1")), r("0"));
    assert_eq!(p.seek(&r("100")), r("2"));
    assert_eq!(p.seek(&r("3/8")), r("3/4"));
    assert!(schedule.seconds(&r("-1")).is_err());
    assert!(schedule.seconds(&r("3")).is_err());
    let d = schedule.discovery();
    assert_eq!(d.authored.len(), 2);
    assert_eq!(d.occurrences.len(), 2);
    for (o, (id, order, slash)) in d
        .occurrences
        .iter()
        .zip([("z-grace", -2, None), ("a-grace", -1, Some(true))])
    {
        assert_eq!(o.written_id.0, id);
        assert_eq!(o.order, order);
        assert_eq!(o.slash, slash);
        assert_eq!(o.written_principal.0, "principal");
        assert_eq!(o.written_anchor_quarter, r("1/2"));
        assert_eq!(o.anchor_quarter, r("1/2"));
        assert_eq!(o.written_principal_start, r("1/2"));
        assert_eq!(o.written_principal_end, r("2"));
        assert_eq!(o.principal_start, r("1/2"));
        assert_eq!(o.principal_end, r("2"));
        assert_eq!(o.measure_occurrence, 0);
        assert_eq!(o.timing, scorestream_core::score::GraceTiming::default());
    }
    // Qualified projection retains Duration::Grace and nominal principal timing.
    // Its sidecar owns all allocated sound time; there is no forged measured note.
    let doc = scorestream_codec::parse(&s).unwrap();
    let qualified = perform_document_with_explicit_graces(
        &doc,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
    )
    .unwrap();
    for o in &d.occurrences {
        let event = qualified
            .performed()
            .score
            .events
            .iter()
            .find(|e| e.id == o.id)
            .unwrap();
        assert_eq!(event.duration, Duration::Grace);
        assert_eq!(event.start, r("1/2"));
        assert_eq!(event.end(), r("1/2"));
        assert_eq!(event.grace.as_ref().unwrap().anchor, o.principal);
        let principal = qualified
            .performed()
            .score
            .events
            .iter()
            .find(|e| e.id == o.principal)
            .unwrap();
        assert_eq!(principal.start, r("1/2"));
        assert_eq!(principal.end(), r("2"));
        assert_eq!(event.part, principal.part);
        assert_eq!(event.voice, principal.voice);
        assert_eq!(event.staff, principal.staff);
    }
}

#[test]
fn authored_percent_receipt_and_exact_release_are_not_an_explicit_quarter_guess() {
    let s = singleton(" steal-time-following=\"25\"", 1, "");
    let p = play(&s, &authored(&s)).unwrap();
    assert_eq!(
        p.schedule().groups()[0].allocation,
        GraceAllocation::AuthoredFollowingPercent
    );
    assert_eq!(p.schedule().groups()[0].allocated_quarters, r("1/4"));
    assert_eq!(p.schedule().windows()[0].quarter_start, r("0"));
    assert_eq!(p.schedule().windows()[0].quarter_end, r("1/4"));
    assert_eq!(p.schedule().windows()[1].quarter_start, r("1/4"));
    assert_eq!(p.schedule().windows()[1].quarter_end, r("47/50"));
    assert_eq!(p.schedule().windows()[1].seconds_start, r("1/8"));
    assert_eq!(p.schedule().windows()[1].seconds_end, r("47/100"));
    assert_eq!(
        p.schedule().discovery().authored[0].timing.steal_following,
        Some(r("25"))
    );
}

#[test]
fn repeated_graces_rebind_principals_and_restore_tempo_dynamics_pitch_and_instrument() {
    let mut imported = include_str!("fixtures/navigation-performed.musicxml").to_owned();
    for id in ["before", "a", "b", "c", "d"] {
        imported = imported.replace(
            &format!("<note id=\"{id}\">"),
            &format!(
                "{}<note id=\"{id}\">",
                grace(&format!("grace-{id}"), "", "D")
            ),
        );
    }
    let original = source(&imported);
    // A typed SFS instrument state at the first-ending entry exercises the same
    // supported checkpoint machinery as the imported pitch/tempo/dynamic states.
    assert!(original.contains("\n@2\n"));
    let s = original.replace("\n@2\n", "\n@2\n= P1 instrument=flute\n");
    let d = discover_source_graces(&s).unwrap();
    assert_eq!(d.authored.len(), 5);
    assert_eq!(d.occurrences.len(), 6);
    let repeated = d
        .occurrences
        .iter()
        .filter(|o| o.written_id.0 == "grace-a")
        .collect::<Vec<_>>();
    assert_eq!(repeated.len(), 2);
    assert_ne!(repeated[0].id, repeated[1].id);
    assert_ne!(repeated[0].principal, repeated[1].principal);
    assert_eq!(repeated[0].anchor_quarter, r("1"));
    assert_eq!(repeated[1].anchor_quarter, r("3"));
    assert_eq!(repeated[0].written_anchor_quarter, r("1"));
    assert_eq!(repeated[1].written_anchor_quarter, r("1"));
    assert_eq!(
        d.authored
            .iter()
            .find(|a| a.written_id.0 == "grace-a")
            .unwrap()
            .occurrence_ids
            .len(),
        2
    );
    assert_eq!(
        d.authored
            .iter()
            .find(|a| a.written_id.0 == "grace-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1,
        "The first ending is skipped on the second visit, not replayed or erased from the source audit"
    );
    let policy = ExplicitGracePolicy {
        groups: d
            .occurrences
            .iter()
            .map(|o| ExplicitGraceGroup {
                principal: o.principal.clone(),
                allocation: GraceAllocation::ExplicitFollowingQuarters {
                    members: vec![GraceQuarterAllocation {
                        event: o.id.clone(),
                        quarters: if o.measure_occurrence == 3 {
                            r("1/2")
                        } else {
                            r("1/4")
                        },
                    }],
                },
            })
            .collect(),
    };
    let p = play(&s, &policy).unwrap();
    assert_eq!(p.plan().notes.len(), 12);
    let grace_ids = [
        "grace-before",
        "grace-a",
        "grace-b",
        "grace-a",
        "grace-c",
        "grace-d",
    ];
    let principal_ids = ["before", "a", "b", "a", "c", "d"];
    for i in 0..p.plan().notes.len() / 2 {
        let pair = &p.plan().notes[i * 2..i * 2 + 2];
        assert_eq!(pair[0].written_id.as_deref(), Some(grace_ids[i]));
        assert_eq!(pair[1].written_id.as_deref(), Some(principal_ids[i]));
        close(pair[0].start_seconds, [0., 1., 2., 2.5, 3.5, 4.5][i]);
        close(
            pair[0].duration_seconds,
            [0.25, 0.25, 0.125, 0.5, 0.25, 0.25][i],
        );
        close(
            pair[1].start_seconds,
            [0.25, 1.25, 2.125, 3., 3.75, 4.75][i],
        );
        close(pair[0].midi_note, [72., 72., 71., 72., 72., 72.][i]);
        close(pair[1].midi_note, [57., 58., 59., 58., 62., 63.][i]);
        assert_eq!(pair[0].velocity, [45, 45, 90, 45, 45, 45][i]);
        assert_eq!(pair[0].velocity, pair[1].velocity);
        let instrument = if i == 2 { "flute" } else { "piano" };
        assert_eq!(pair[0].instrument, instrument);
        assert_eq!(pair[1].instrument, instrument);
    }
    close(p.plan().duration_seconds, 5.5);
    assert_eq!(p.schedule().duration_seconds(), r("11/2"));
    let mut written_instead = policy.clone();
    written_instead.groups[1].principal = EventId("a".into());
    reject(&s, &written_instead, "grace.policy_principal");
}

fn at_frame(source: &str, at: &str, records: &str) -> String {
    let frame = format!("\n@{at}\n");
    assert!(source.contains(&frame), "missing source frame {at}");
    source.replace(&frame, &format!("{frame}{records}\n"))
}
#[test]
fn affected_incoming_and_outgoing_ties_and_articulations_reject_in_core_and_app() {
    let s = source(&xml(
        &format!(
            "{}{}{}{}",
            note("pre", 8, "G", ""),
            grace("g", "", "D"),
            note("main", 8, "G", ""),
            note("tail", 8, "G", "")
        ),
        3,
    ));
    let policy = explicit(&s, &[("g", "1/4")]);
    let incoming = at_frame(
        &at_frame(&s, "0", "~ tie incoming start=#pre"),
        "1",
        "~ tie incoming end=#main",
    );
    let outgoing = at_frame(
        &at_frame(&s, "1", "~ tie outgoing start=#main"),
        "2",
        "~ tie outgoing end=#tail",
    );
    let principal_articulation = at_frame(&s, "1", "+ #main articulation value=staccato");
    let grace_articulation = at_frame(&s, "1", "+ #g articulation value=accent");
    for (source, code) in [
        (incoming, "grace.relation_unsupported"),
        (outgoing, "grace.relation_unsupported"),
        (principal_articulation, "grace.articulation_unsupported"),
        (grace_articulation, "grace.articulation_unsupported"),
    ] {
        let doc = scorestream_codec::parse(&source).unwrap();
        let err = perform_document_with_explicit_graces(
            &doc,
            &policy,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            PerformanceLimits::default(),
            GraceTimingLimits::default(),
        )
        .unwrap_err();
        assert!(err.iter().any(|d| d.code == code), "{err:?}");
        reject(&source, &policy, code);
    }
}

#[test]
fn unrelated_normal_tie_chain_remains_one_attack_but_mixed_cue_chain_rejects() {
    let s = source(&xml(
        &format!(
            "{}{}{}{}{}",
            grace("g", "", "D"),
            note("main", 8, "G", ""),
            note("a", 8, "A", ""),
            note("b", 8, "A", ""),
            note("c", 8, "A", "")
        ),
        4,
    ));
    let s = at_frame(
        &at_frame(
            &at_frame(&s, "1", "~ tie chain start=#a"),
            "2",
            "~ tie chain continue=#b",
        ),
        "3",
        "~ tie chain end=#c",
    );
    let p = play(&s, &explicit(&s, &[("g", "1/4")])).unwrap();
    assert_eq!(p.plan().notes.len(), 3);
    close(p.plan().notes[2].start_seconds, 0.5);
    close(p.plan().notes[2].duration_seconds, 1.46);
    for id in ["a", "b", "c"] {
        let cue = s.replace(&format!("id={id}"), &format!("id={id} kind=cue"));
        let policy = explicit(&cue, &[("g", "1/4")]);
        reject(&cue, &policy, "grace.mixed_cue_tie");
    }
}

#[test]
fn numeric_attack_boundaries_use_actual_start_and_terminal_controls_do_not_leak() {
    // At q=1/4, a grace ends exactly as its principal attacks. A q=47/50
    // terminal numeric change must not alter the already-released principal.
    let s = singleton(
        "",
        1,
        "<sound dynamics=\"50\"/><sound dynamics=\"100\"><offset>2</offset></sound><sound dynamics=\"25\"><offset>7.52</offset></sound>",
    );
    let p = play(&s, &explicit(&s, &[("g", "1/4")])).unwrap();
    assert_eq!(p.plan().notes[0].velocity, 45);
    assert_eq!(p.plan().notes[1].velocity, 90);
    assert!(
        p.plan().notes[0]
            .gain_envelope
            .iter()
            .all(|point| point.gain == 1.)
    );
    assert!(
        p.plan().notes[1]
            .gain_envelope
            .iter()
            .all(|point| point.gain == 1.)
    );
}

#[test]
fn precision_and_exact_rational_and_count_budgets_fail_without_partial_plan() {
    let s = singleton("", 1, "");
    for amount in [
        "1/10000000",
        "9999999999/10000000000",
        "1/1000000000000000000000000000000",
    ] {
        reject(&s, &explicit(&s, &[("g", amount)]), "grace.clock_precision");
    }
    let doc = scorestream_codec::parse(&s).unwrap();
    let policy = explicit(&s, &[("g", "1/4")]);
    for limits in [
        GraceTimingLimits {
            maximum_source_records: 1,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_events: 1,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_controls: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_groups: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_members: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_rational_bits: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_timeline_seconds: r("1/4"),
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_timeline_seconds: r("21601"),
            ..Default::default()
        },
    ] {
        assert!(
            perform_document_with_explicit_graces(
                &doc,
                &policy,
                GraceSoundPolicy::FullGraceThenGatedRemainder,
                PerformanceLimits::default(),
                limits
            )
            .is_err()
        );
    }
    let precise = explicit(&s, &[("g", "1/1024")]);
    let limits = GraceTimingLimits {
        maximum_rational_bits: 8,
        maximum_timeline_seconds: r("1"),
        ..Default::default()
    };
    let errors = perform_document_with_explicit_graces(
        &doc,
        &precise,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        limits,
    )
    .unwrap_err();
    assert!(errors.iter().any(|d| d.code == "grace.rational_budget"));
    let repeat_limits = PerformanceLimits {
        maximum_occurrences: 0,
        ..Default::default()
    };
    assert!(
        perform_document_with_explicit_graces(
            &doc,
            &policy,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            repeat_limits,
            GraceTimingLimits::default()
        )
        .is_err()
    );
}

#[test]
fn delayed_attacks_keep_source_resolved_pitch_staff_and_instrument() {
    let imported = include_str!("fixtures/grace-playback.musicxml").replace("</part>", "<measure id=\"later\" number=\"2\"><note id=\"after\"><pitch><step>C</step><octave>4</octave></pitch><duration>16</duration><voice>1</voice><type>half</type></note></measure></part>");
    let original = source(&imported);
    let voice = scorestream_codec::parse(&original).unwrap().voices[0]
        .id
        .clone();
    // A later state change is not a license to reinterpret a source event whose
    // nominal onset is earlier. Only numeric dynamics follows its delayed attack.
    let s = original.replace("staves=1", "staves=2").replace("\n@3/4\n", &format!("\n@9/16\n= p instrument=flute transpose_chromatic=12 transpose_diatonic=7 transpose_octaves=0\n= {voice} staff=2\n.\n@3/4\n"));
    assert!(s.contains("@9/16"));
    let policy = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let p = play(&s, &policy).unwrap();
    for (n, midi) in p.plan().notes.iter().zip([74., 76., 67.]) {
        close(n.midi_note, midi);
        assert_eq!(n.instrument, "piano");
        assert_eq!(n.voice, voice);
    }
    assert_eq!(p.plan().notes[2].velocity, 90);
    assert_eq!(p.plan().routes.len(), 2);
    let after = p
        .plan()
        .notes
        .iter()
        .find(|n| n.written_id.as_deref() == Some("after"))
        .unwrap();
    assert_eq!(after.instrument, "flute");
    close(after.midi_note, 72.);
    close(after.start_seconds, 13. / 8.);
    let doc = scorestream_codec::parse(&s).unwrap();
    let qualified = perform_document_with_explicit_graces(
        &doc,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
    )
    .unwrap();
    assert!(
        qualified
            .performed()
            .score
            .events
            .iter()
            .filter(|e| e.start < r("2"))
            .all(|e| e.staff.number == 1 && e.instrument.0 == "piano")
    );
}

#[test]
fn unrelated_overlapping_voice_keeps_its_exact_existing_gate_and_route() {
    let imported = include_str!("fixtures/grace-playback.musicxml").replace("</measure>", "<backup><duration>16</duration></backup><note id=\"parallel\"><pitch><step>C</step><octave>4</octave></pitch><duration>16</duration><voice>2</voice><type>half</type></note></measure>");
    let model = MusicXml.read(&imported).unwrap().value;
    let before = model.clone();
    let s = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(model, before);
    // SFS groups simultaneous voices chronologically. Only the unrelated
    // measured-event container order differs; compare every typed event by ID.
    // Authored grace order, identities, timing and all other model fields remain
    // exact. No normalized model or rewritten source is sent to playback.
    let keyed = |mut document: scorestream_core::score::ScoreDocument| {
        for part in &mut document.parts {
            for measure in &mut part.measures {
                measure.events.sort_by(|a, b| a.id.cmp(&b.id));
            }
        }
        document
    };
    assert_eq!(
        keyed(SfsAdapter.read(&s).unwrap().value),
        keyed(model.clone())
    );
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value,
        model
    );
    let p = play(&s, &explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")])).unwrap();
    assert_eq!(p.plan().notes.len(), 4);
    let parallel = p
        .plan()
        .notes
        .iter()
        .find(|n| n.written_id.as_deref() == Some("parallel"))
        .unwrap();
    close(parallel.start_seconds, 0.);
    close(parallel.duration_seconds, 293. / 200.);
    close(parallel.midi_note, 60.);
    assert_eq!(parallel.velocity, 45);
    assert_eq!(parallel.instrument, "piano");
    assert_eq!(parallel.part, "p");
    assert!(
        parallel
            .gain_envelope
            .windows(2)
            .any(|w| w[0].offset_seconds == 0.375
                && w[1].offset_seconds == 0.375
                && w[0].gain == 1.
                && w[1].gain == 2.)
    );
    assert!(
        p.schedule()
            .windows()
            .iter()
            .all(|w| w.id != EventId(parallel.id.clone()))
    );
    let marker = p
        .plan()
        .event_positions
        .iter()
        .find(|e| e.id == "parallel")
        .unwrap();
    close(marker.start_seconds, 0.);
    close(marker.end_seconds, 13. / 8.);
    close(p.plan().duration_seconds, 13. / 8.);
}

#[test]
fn direct_malformed_documents_and_noise_return_diagnostics_without_a_partial_schedule() {
    let s = singleton("", 1, "");
    let original = scorestream_codec::parse(&s).unwrap();
    let policy = explicit(&s, &[("g", "1/4")]);
    for change in 0..6 {
        let mut doc = original.clone();
        for frame in &mut doc.frames {
            match change {
                0 => frame.records.retain(|record| !matches!(record, scorestream_core::Record::Mark(mark) if mark.kind == "scorestream.grace")),
                1 => frame.records.retain(|record| !matches!(record, scorestream_core::Record::Event(event) if event.id() == Some("main"))),
                _ => (),
            }
            for record in &mut frame.records {
                if let scorestream_core::Record::Event(event) = record {
                    if event.id() != Some("g") {
                        continue;
                    }
                    match change {
                        2 => {
                            event.attributes.remove("anchor");
                        }
                        3 => {
                            event.attributes.insert(
                                "anchor".into(),
                                scorestream_core::Scalar::String("#missing".into()),
                            );
                        }
                        4 => {
                            event.attributes.insert(
                                "order".into(),
                                scorestream_core::Scalar::String("wrong-type".into()),
                            );
                        }
                        5 => {
                            event.pitch = scorestream_core::Pitch::Unpitched("noise".into());
                        }
                        _ => (),
                    }
                }
            }
        }
        let discovery = scorestream_core::notation::discover_document_graces(&doc).unwrap_err();
        assert!(!discovery.is_empty());
        let errors = perform_document_with_explicit_graces(
            &doc,
            &policy,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            PerformanceLimits::default(),
            GraceTimingLimits::default(),
        )
        .unwrap_err();
        assert!(!errors.is_empty());
    }
}

#[test]
fn grace_allocations_cannot_borrow_a_member_from_another_principal() {
    let s = source(&xml(
        &format!(
            "{}{}{}{}",
            grace("g1", "", "D"),
            note("main1", 8, "G", ""),
            grace("g2", "", "E"),
            note("main2", 8, "A", "")
        ),
        2,
    ));
    let mut p = explicit(&s, &[("g1", "1/4"), ("g2", "1/4")]);
    let GraceAllocation::ExplicitFollowingQuarters { members: other } = &p.groups[1].allocation
    else {
        unreachable!()
    };
    let foreign = other[0].event.clone();
    let GraceAllocation::ExplicitFollowingQuarters { members } = &mut p.groups[0].allocation else {
        unreachable!()
    };
    members[0].event = foreign;
    reject(&s, &p, "grace.policy_member");
    let mut missing = explicit(&s, &[("g1", "1/4"), ("g2", "1/4")]);
    missing.groups.pop();
    reject(&s, &missing, "grace.policy_missing");
}
