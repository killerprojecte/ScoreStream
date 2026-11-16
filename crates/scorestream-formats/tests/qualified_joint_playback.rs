//! Original synthetic MusicXML → shared model → unchanged SFS → joint playback.
//! These finite source-qualified contracts are not an original-score acceptance,
//! a repaired Beethoven excerpt, or a claim about sampled audio quality.
use scorestream_app::playback::{
    ExplicitCombinedPlayback, FermataSoundPolicy, GraceSoundPolicy, discover_source_fermatas,
    discover_source_graces, discover_source_graces_and_fermatas, plan_source,
    plan_source_with_explicit_fermatas, plan_source_with_explicit_graces,
    plan_source_with_explicit_graces_and_fermatas,
};
use scorestream_core::{
    Duration, Rational,
    fermata_timing::{ExplicitFermataPolicy, ExplicitHoldGroup, SeekPosition, TimingLimits},
    notation::{
        EventId, EventRole, ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation,
        GraceQuarterAllocation, GraceTimingLimits, PerformanceLimits, lower_document,
        perform_document, perform_document_with_explicit_fermatas,
        perform_document_with_explicit_graces, perform_document_with_explicit_graces_and_fermatas,
    },
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
use std::collections::BTreeMap;

const XML: &str = include_str!("fixtures/joint-playback.musicxml");

fn r(value: &str) -> Rational {
    value.parse().unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}
fn note(id: &str, ticks: usize, pitch: &str, notation: &str) -> String {
    let pitch = if pitch == "rest" {
        "<rest/>".into()
    } else {
        format!("<pitch><step>{pitch}</step><octave>4</octave></pitch>")
    };
    format!(
        "<note id=\"{id}\">{pitch}<duration>{ticks}</duration><voice>1</voice>{notation}</note>"
    )
}
fn grace(id: &str, attrs: &str) -> String {
    format!(
        "<note id=\"{id}\"><grace{attrs}/><pitch><step>D</step><octave>5</octave></pitch><voice>1</voice><type>16th</type></note>"
    )
}
fn xml(parts: &[String], quarters: usize) -> String {
    let list = (0..parts.len()).map(|i| format!("<score-part id=\"p{i}\"><part-name>Piano</part-name><score-instrument id=\"i{i}\"><instrument-name>piano</instrument-name></score-instrument></score-part>")).collect::<String>();
    let bodies = parts.iter().enumerate().map(|(i, body)| format!("<part id=\"p{i}\"><measure id=\"m{i}\" number=\"1\"><attributes><divisions>8</divisions><time><beats>{quarters}</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part>")).collect::<String>();
    format!(
        "<score-partwise version=\"4.0\"><part-list>{list}</part-list>{bodies}</score-partwise>"
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
fn single(attrs: &str, controls: &str) -> String {
    source(&xml(
        &[format!(
            "<sound tempo=\"120\"/>{}{}{}{}",
            note("held", 8, "C", "<notations><fermata/></notations>"),
            controls,
            grace("g", attrs),
            note("main", 8, "G", ""),
        )],
        2,
    ))
}
fn explicit(source: &str, allocations: &[(&str, &str)]) -> ExplicitGracePolicy {
    let d = discover_source_graces_and_fermatas(source).unwrap();
    let mut groups = BTreeMap::<EventId, Vec<GraceQuarterAllocation>>::new();
    for o in d.graces.occurrences {
        let amount = allocations
            .iter()
            .find(|(id, _)| *id == o.written_id.0)
            .unwrap()
            .1;
        groups
            .entry(o.principal)
            .or_default()
            .push(GraceQuarterAllocation {
                event: o.id,
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
fn holds(source: &str, boundaries: &[(&str, &str)]) -> ExplicitFermataPolicy {
    let d = discover_source_graces_and_fermatas(source).unwrap();
    ExplicitFermataPolicy {
        groups: boundaries
            .iter()
            .enumerate()
            .map(|(i, (quarter, seconds))| ExplicitHoldGroup {
                id: format!("explicit-{i}"),
                members: d
                    .fermatas
                    .occurrences
                    .iter()
                    .filter(|o| o.quarter_end == r(quarter))
                    .map(|o| o.id.clone())
                    .collect(),
                added_seconds: r(seconds),
            })
            .collect(),
    }
}
fn play(
    source: &str,
    grace: &ExplicitGracePolicy,
    fermata: &ExplicitFermataPolicy,
) -> Result<ExplicitCombinedPlayback, String> {
    let text = source.to_owned();
    let before = scorestream_codec::parse(source).unwrap();
    let result = plan_source_with_explicit_graces_and_fermatas(
        source,
        grace,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        fermata,
        FermataSoundPolicy::SustainSelectedAnchors,
    );
    assert_eq!(source, text);
    assert_eq!(scorestream_codec::parse(source).unwrap(), before);
    result
}
fn reject(source: &str, grace: &ExplicitGracePolicy, fermata: &ExplicitFermataPolicy, code: &str) {
    let error = play(source, grace, fermata).unwrap_err();
    assert!(error.contains(code), "expected {code}, got {error}");
}
fn at_frame(source: &str, at: &str, records: &str) -> String {
    let frame = format!("\n@{at}\n");
    assert!(source.contains(&frame), "missing frame {at}");
    source.replace(&frame, &format!("{frame}{records}\n"))
}

#[test]
fn final_clock_windows_and_numeric_controls_share_one_real_playback_plan() {
    let s = source(XML);
    assert!(s.contains("!requires scorestream.grace@1"));
    assert!(s.contains("!requires scorestream.fermata@1"));
    let mut policy = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    // Policy container order is not authored order, nor lexicographic event ID.
    if let GraceAllocation::ExplicitFollowingQuarters { members } = &mut policy.groups[0].allocation
    {
        members.reverse();
    }
    let p = play(&s, &policy, &holds(&s, &[("1", "1")])).unwrap();
    assert_eq!(p.plan().notes.len(), 4);
    for (n, (pitch, start, duration, velocity)) in p.plan().notes.iter().zip([
        (60., 0., 1.5, 45),
        (74., 1.5, 0.125, 90),
        (76., 1.625, 0.1875, 90),
        (67., 1.8125, 0.7475, 45),
    ]) {
        close(n.midi_note, pitch);
        close(n.start_seconds, start);
        close(n.duration_seconds, duration);
        assert_eq!(n.velocity, velocity);
        assert_eq!(n.instrument, "piano");
    }
    // The terminal selected note keeps the old level through the entire hold.
    assert!(
        p.plan().notes[0]
            .gain_envelope
            .iter()
            .all(|point| point.gain == 1.)
    );
    let second = &p.plan().notes[2];
    assert!(second.gain_envelope.windows(2).any(|pair| {
        (pair[0].offset_seconds - 0.125).abs() < 1e-9
            && pair[0].offset_seconds == pair[1].offset_seconds
            && pair[0].gain == 1.
            && pair[1].gain == 0.5
    }));
    assert!(
        p.plan().notes[3]
            .gain_envelope
            .iter()
            .all(|point| point.gain == 1.)
    );
    close(p.plan().duration_seconds, 21. / 8.);
    close(p.plan().measures[0].end_seconds, 21. / 8.);
    assert_eq!(p.timing().duration_seconds(), &r("21/8"));
    assert_eq!(p.timing().quarter_end(), &r("3"));
    assert_eq!(p.grace_groups()[0].allocated_quarters, r("3/8"));
    assert_eq!(p.grace_groups()[0].allocation, policy.groups[0].allocation);
    for (window, (role, q0, q1, s0, s1)) in p.grace_windows().iter().zip([
        (EventRole::Grace, "1", "9/8", "3/2", "13/8"),
        (EventRole::Grace, "9/8", "11/8", "13/8", "29/16"),
        (EventRole::Note, "11/8", "287/100", "29/16", "64/25"),
    ]) {
        assert_eq!(window.role, role);
        assert_eq!(window.quarter_start, r(q0));
        assert_eq!(window.quarter_end, r(q1));
        assert_eq!(window.seconds_start, r(s0));
        assert_eq!(window.seconds_end, r(s1));
        assert_eq!(p.timing().after(&r(q0)), r(s0));
        assert_eq!(p.timing().after(&r(q1)), r(s1));
        assert_eq!(p.seek(&r(s0)), SeekPosition::Music { quarter: r(q0) });
        assert_eq!(p.seek(&r(s1)), SeekPosition::Music { quarter: r(q1) });
    }
    assert!(
        !p.plan()
            .diagnostics
            .iter()
            .any(|d| d.code == "grace_unrealized")
    );
}

#[test]
fn hold_seek_is_half_open_and_clamped_on_the_exact_combined_clock() {
    let s = source(XML);
    let p = play(
        &s,
        &explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]),
        &holds(&s, &[("1", "1")]),
    )
    .unwrap();
    for (seconds, elapsed) in [("1/2", "0"), ("1", "1/2"), ("11/8", "7/8")] {
        assert_eq!(
            p.seek(&r(seconds)),
            SeekPosition::Hold {
                group_id: "explicit-0".into(),
                boundary: r("1"),
                elapsed_seconds: r(elapsed),
            }
        );
    }
    assert_eq!(p.timing().before(&r("1")), r("1/2"));
    assert_eq!(p.timing().after(&r("1")), r("3/2"));
    for (seconds, quarter) in [
        ("3/2", "1"),
        ("27/16", "19/16"),
        ("-1", "0"),
        ("21/8", "3"),
        ("100", "3"),
    ] {
        assert_eq!(
            p.seek(&r(seconds)),
            SeekPosition::Music {
                quarter: r(quarter)
            }
        );
    }
}

#[test]
fn source_audits_and_written_markers_remain_unmeasured_and_nominal() {
    let s = source(XML);
    let policy = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let hold = holds(&s, &[("1", "1")]);
    let p = play(&s, &policy, &hold).unwrap();
    let doc = scorestream_codec::parse(&s).unwrap();
    let before = doc.clone();
    let qualified = perform_document_with_explicit_graces_and_fermatas(
        &doc,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &hold,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
        TimingLimits::default(),
    )
    .unwrap();
    assert_eq!(doc, before);
    // The public snapshot clock does not inherit this opaque constructor's
    // qualification. Passing its performed receipt still cannot bypass graces.
    let raw = scorestream_core::fermata_timing::interpret(
        &doc,
        qualified.performed(),
        &hold,
        TimingLimits::default(),
    )
    .unwrap_err();
    assert_eq!(raw.code, "fermata.unsupported_grace");
    assert_eq!(
        doc.events()
            .filter(|(_, e)| e.duration == Duration::Grace)
            .count(),
        2
    );
    let discovery = p.grace_discovery();
    assert_eq!(discovery.authored.len(), 2);
    assert_eq!(discovery.occurrences.len(), 2);
    for (o, (id, order, slash)) in discovery
        .occurrences
        .iter()
        .zip([("z-grace", -2, None), ("a-grace", -1, Some(true))])
    {
        assert_eq!(o.written_id.0, id);
        assert_eq!(o.order, order);
        assert_eq!(o.slash, slash);
        assert_eq!(o.written_principal.0, "principal");
        assert_eq!(o.principal_start, r("1"));
        assert_eq!(o.principal_end, r("3"));
        assert_eq!(o.timing, scorestream_core::score::GraceTiming::default());
        let event = qualified
            .performed()
            .score
            .events
            .iter()
            .find(|e| e.id == o.id)
            .unwrap();
        assert_eq!(event.duration, Duration::Grace);
        assert_eq!(event.start, r("1"));
        assert_eq!(event.end(), r("1"));
        assert_eq!(event.grace.as_ref().unwrap().anchor, o.principal);
        let marker = p
            .plan()
            .event_positions
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        close(marker.start_seconds, 1.5);
        close(marker.end_seconds, 1.5);
    }
    let principal = qualified
        .performed()
        .score
        .events
        .iter()
        .find(|e| e.id == discovery.occurrences[0].principal)
        .unwrap();
    assert_eq!(principal.start, r("1"));
    assert_eq!(principal.end(), r("3"));
    let marker = p
        .plan()
        .event_positions
        .iter()
        .find(|e| e.id == "principal")
        .unwrap();
    close(marker.start_seconds, 1.5);
    close(marker.end_seconds, 2.625);
    assert_eq!(p.timing().authored.len(), 1);
    assert_eq!(p.timing().fermatas[0].written_id.0, "held");
}

#[test]
fn ordinary_and_single_feature_entrypoints_still_reject_both_capabilities() {
    let s = source(XML);
    let gp = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let fp = holds(&s, &[("1", "1")]);
    let d = scorestream_codec::parse(&s).unwrap();
    assert!(lower_document(&d).is_err());
    assert!(perform_document(&d, PerformanceLimits::default()).is_err());
    assert!(plan_source(&s).is_err());
    assert!(discover_source_graces(&s).is_err());
    assert!(discover_source_fermatas(&s).is_err());
    assert!(
        plan_source_with_explicit_graces(&s, &gp, GraceSoundPolicy::FullGraceThenGatedRemainder)
            .is_err()
    );
    assert!(
        plan_source_with_explicit_fermatas(&s, &fp, FermataSoundPolicy::SustainSelectedAnchors)
            .is_err()
    );
    assert!(
        perform_document_with_explicit_graces(
            &d,
            &gp,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            PerformanceLimits::default(),
            GraceTimingLimits::default()
        )
        .is_err()
    );
    assert!(
        perform_document_with_explicit_fermatas(
            &d,
            &fp,
            PerformanceLimits::default(),
            TimingLimits::default()
        )
        .is_err()
    );
}

#[test]
fn missing_duplicate_unknown_or_nonpositive_policy_entries_reject() {
    let s = source(XML);
    let gp = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let fp = holds(&s, &[("1", "1")]);
    reject(&s, &ExplicitGracePolicy { groups: vec![] }, &fp, "grace.");
    reject(
        &s,
        &gp,
        &ExplicitFermataPolicy { groups: vec![] },
        "fermata.",
    );
    for amount in ["0", "-1/8", "2"] {
        reject(
            &s,
            &explicit(&s, &[("z-grace", amount), ("a-grace", "1/4")]),
            &fp,
            "grace.",
        );
    }
    for edit in 0..4 {
        let mut bad = gp.clone();
        if let GraceAllocation::ExplicitFollowingQuarters { members } =
            &mut bad.groups[0].allocation
        {
            match edit {
                0 => {
                    members.pop();
                }
                1 => members.push(members[0].clone()),
                2 => members[0].event = EventId("unknown-grace".into()),
                _ => bad.groups[0].principal = EventId("unknown-principal".into()),
            }
        }
        reject(&s, &bad, &fp, "grace.policy_");
    }
    let mut duplicate = gp.clone();
    duplicate.groups.push(gp.groups[0].clone());
    reject(&s, &duplicate, &fp, "grace.policy_duplicate");
    for edit in 0..4 {
        let mut bad = fp.clone();
        match edit {
            0 => bad.groups[0].added_seconds = r("0"),
            1 => bad.groups[0].added_seconds = r("-1"),
            2 => {
                let member = bad.groups[0].members[0].clone();
                bad.groups[0].members.push(member);
            }
            _ => bad.groups[0].members[0] = EventId("unknown-anchor".into()),
        }
        reject(&s, &gp, &bad, "fermata.");
    }
}

#[test]
fn invalid_source_and_unknown_capabilities_do_not_produce_partial_playback() {
    let s = source(XML);
    let gp = explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let fp = holds(&s, &[("1", "1")]);
    for bad in [
        s.replace(
            "!requires scorestream.grace@1",
            "!requires scorestream.grace@1\n!requires scorestream.unknown-test@1",
        ),
        s.replace("anchor=#principal", "anchor=#missing"),
        s.replace("id=a-grace", "id=z-grace"),
    ] {
        assert_ne!(bad, s);
        assert!(discover_source_graces_and_fermatas(&bad).is_err());
        assert!(
            plan_source_with_explicit_graces_and_fermatas(
                &bad,
                &gp,
                GraceSoundPolicy::FullGraceThenGatedRemainder,
                &fp,
                FermataSoundPolicy::SustainSelectedAnchors
            )
            .is_err()
        );
    }
}

#[test]
fn authored_singleton_percent_is_accepted_only_over_constant_effective_tempo() {
    let s = single(" slash=\"yes\" steal-time-following=\"25\"", "");
    let occurrence = &discover_source_graces_and_fermatas(&s)
        .unwrap()
        .graces
        .occurrences[0];
    let gp = ExplicitGracePolicy {
        groups: vec![ExplicitGraceGroup {
            principal: occurrence.principal.clone(),
            allocation: GraceAllocation::AuthoredFollowingPercent,
        }],
    };
    let p = play(&s, &gp, &holds(&s, &[("1", "1")])).unwrap();
    assert_eq!(p.grace_groups()[0].allocated_quarters, r("1/4"));
    assert_eq!(p.grace_windows()[0].seconds_start, r("3/2"));
    assert_eq!(p.grace_windows()[0].seconds_end, r("13/8"));
    assert_eq!(
        p.grace_discovery().authored[0].timing.steal_following,
        Some(r("25"))
    );
    let variable = single(
        " steal-time-following=\"25\"",
        "<sound tempo=\"60\"><offset>4</offset></sound>",
    );
    reject(
        &variable,
        &gp,
        &holds(&variable, &[("1", "1")]),
        "grace.percentage_tempo_ambiguous",
    );
    let unchanged = single(
        " steal-time-following=\"25\"",
        "<sound tempo=\"120\"><offset>4</offset></sound>",
    );
    assert!(play(&unchanged, &gp, &holds(&unchanged, &[("1", "1")])).is_ok());
}

#[test]
fn unsupported_authored_hints_cannot_be_overridden_by_explicit_quarters() {
    for attrs in [
        " steal-time-following=\"0\"",
        " steal-time-following=\"100\"",
        " steal-time-following=\"25\"",
        " steal-time-previous=\"25\"",
        " make-time=\"1\"",
        " steal-time-following=\"25\" steal-time-previous=\"0\"",
        " steal-time-following=\"25\" make-time=\"0\"",
    ] {
        let s = single(attrs, "");
        reject(
            &s,
            &explicit(&s, &[("g", "1/4")]),
            &holds(&s, &[("1", "1")]),
            "grace.",
        );
    }
}

#[test]
fn selected_fermata_on_a_grace_or_its_principal_is_not_a_supported_combination() {
    for target in ["g", "main"] {
        let imported = xml(
            &[format!("{}{}", grace("g", ""), note("main", 8, "G", ""))],
            1,
        );
        let marker = if target == "g" {
            "<type>16th</type>"
        } else {
            "<duration>8</duration><voice>1</voice>"
        };
        let imported = imported.replace(
            marker,
            &format!("{marker}<notations><fermata/></notations>"),
        );
        if target == "g" {
            // Authored signs on unmeasured graces already reject at the genuine
            // import boundary. Do not erase or move the sign to create a test.
            assert!(MusicXml.read(&imported).is_err());
            continue;
        }
        let s = source(&imported);
        let gp = explicit(&s, &[("g", "1/4")]);
        let fp = holds(&s, &[(if target == "g" { "0" } else { "1" }, "1")]);
        reject(&s, &gp, &fp, "combined.affected_fermata_anchor");
    }
}

#[test]
fn global_cross_part_hold_inside_or_at_principal_nominal_end_rejects() {
    for boundary in [2, 3] {
        let lead = note("lead", 8, "rest", "") + &grace("g", "") + &note("main", 16, "G", "");
        // A rest-selected hold in another part has different event, voice and
        // staff identities, but shares the one global performance clock.
        let anchor = note(
            "other",
            boundary * 8,
            "rest",
            "<notations><fermata/></notations>",
        ) + &if boundary < 3 {
            note("other-tail", 8, "rest", "")
        } else {
            String::new()
        };
        let s = source(&xml(&[lead, anchor], 3));
        let gp = explicit(&s, &[("g", "1/4")]);
        let fp = holds(&s, &[(if boundary == 2 { "2" } else { "3" }, "1")]);
        let error = play(&s, &gp, &fp).unwrap_err();
        assert!(error.contains("combined.grace_hold_overlap"), "{error}");
        assert!(
            error.contains("main") && error.contains("other") && error.contains("explicit-0"),
            "{error}"
        );
    }
}

#[test]
fn another_staffs_rest_hold_cannot_split_a_grace_principal() {
    for boundary in [2, 3] {
        let primary = note("lead", 8, "rest", "") + &grace("g", "") + &note("main", 16, "G", "");
        let other = (note(
            "staff-two-anchor",
            boundary * 8,
            "rest",
            "<notations><fermata/></notations>",
        ) + &if boundary == 2 {
            note("staff-two-tail", 8, "rest", "")
        } else {
            String::new()
        })
            .replace("<voice>1</voice>", "<voice>2</voice><staff>2</staff>");
        let imported = xml(&[primary], 3)
            .replace("</attributes>", "<staves>2</staves></attributes>")
            .replace(
                "</measure>",
                &format!("<backup><duration>24</duration></backup>{other}</measure>"),
            );
        let model = MusicXml.read(&imported).unwrap().value;
        let before = model.clone();
        let s = SfsAdapter.write(&model).unwrap().value;
        assert_eq!(model, before);
        // The XML has a serial second voice after backup; SFS emits simultaneous
        // measured events chronologically. Compare those containers by identity
        // only, retaining every field and all authored grace order unchanged.
        let keyed = |mut document: scorestream_core::score::ScoreDocument| {
            for part in &mut document.parts {
                for measure in &mut part.measures {
                    measure.events.sort_by(|a, b| a.id.cmp(&b.id));
                }
            }
            document
        };
        assert_eq!(keyed(SfsAdapter.read(&s).unwrap().value), keyed(model));
        let discovery = discover_source_graces_and_fermatas(&s).unwrap();
        assert_eq!(
            discovery.graces.occurrences[0].part,
            discovery.fermatas.occurrences[0].part
        );
        let doc = scorestream_codec::parse(&s).unwrap();
        assert_eq!(doc.voices.len(), 2);
        assert_eq!(doc.parts[0].staves, 2);
        reject(
            &s,
            &explicit(&s, &[("g", "1/4")]),
            &holds(&s, &[(if boundary == 2 { "2" } else { "3" }, "1")]),
            "combined.grace_hold_overlap",
        );
    }
}

#[test]
fn global_hold_at_principal_onset_is_allowed_and_grace_attacks_after_it() {
    let lead = note("lead", 8, "rest", "") + &grace("g", "") + &note("main", 8, "G", "");
    let anchor = note("other", 8, "rest", "<notations><fermata/></notations>")
        + &note("other-tail", 8, "rest", "");
    let s = source(&xml(&[lead, anchor], 2));
    let p = play(
        &s,
        &explicit(&s, &[("g", "1/4")]),
        &holds(&s, &[("1", "1")]),
    )
    .unwrap();
    assert_eq!(p.plan().notes.len(), 2);
    close(p.plan().notes[0].start_seconds, 1.5);
    close(p.plan().duration_seconds, 2.);
}

#[test]
fn affected_ties_articulations_and_zero_level_at_actual_attack_reject() {
    let s = source(&xml(
        &[format!(
            "{}{}{}{}",
            note("held", 8, "G", "<notations><fermata/></notations>"),
            grace("g", ""),
            note("main", 8, "G", ""),
            note("tail", 8, "G", ""),
        )],
        3,
    ));
    let gp = explicit(&s, &[("g", "1/4")]);
    let fp = holds(&s, &[("1", "1")]);
    // Typed SFS relation/articulation additions cover the lowerer's supported
    // states where this MusicXML adapter does not yet map articulations.
    for (bad, code) in [
        (
            at_frame(
                &at_frame(&s, "0", "~ tie incoming start=#held"),
                "1",
                "~ tie incoming end=#main",
            ),
            "grace.relation_unsupported",
        ),
        (
            at_frame(
                &at_frame(&s, "1", "~ tie outgoing start=#main"),
                "2",
                "~ tie outgoing end=#tail",
            ),
            "grace.relation_unsupported",
        ),
        (
            at_frame(&s, "1", "+ #main articulation value=staccato"),
            "grace.articulation_unsupported",
        ),
        (
            at_frame(&s, "1", "+ #g articulation value=accent"),
            "grace.articulation_unsupported",
        ),
    ] {
        reject(&bad, &gp, &fp, code);
    }
    for controls in [
        "<sound dynamics=\"0\"/>",
        "<sound dynamics=\"100\"/><sound dynamics=\"0\"><offset>2</offset></sound>",
    ] {
        let s = single("", controls);
        reject(
            &s,
            &explicit(&s, &[("g", "1/4")]),
            &holds(&s, &[("1", "1")]),
            "grace.",
        );
    }
}

fn tied_source(selected: usize) -> String {
    let mut body = String::new();
    for i in 0..3 {
        let ties = match i {
            0 => "<tie type=\"start\"/>",
            1 => "<tie type=\"stop\"/><tie type=\"start\"/>",
            _ => "<tie type=\"stop\"/>",
        };
        let marks = format!(
            "{ties}<notations>{}{}</notations>",
            ties.replace("<tie ", "<tied "),
            if i == selected { "<fermata/>" } else { "" }
        );
        body += &note(&format!("tie{i}"), 8, "C", &marks);
    }
    body += &grace("g", "");
    body += &note("main", 8, "G", "");
    body += &note("ordinary", 8, "A", "");
    source(&xml(&[body], 5))
}

#[test]
fn terminal_and_interior_selected_ties_keep_one_attack_and_ordinary_gates() {
    for (selected, boundary, duration) in [(0, "1", 2.46), (1, "2", 2.46), (2, "3", 2.5)] {
        let s = tied_source(selected);
        let p = play(
            &s,
            &explicit(&s, &[("g", "1/4")]),
            &holds(&s, &[(boundary, "1")]),
        )
        .unwrap();
        assert_eq!(p.plan().notes.len(), 4);
        assert_eq!(p.plan().event_positions.len(), 6);
        close(p.plan().notes[0].duration_seconds, duration);
        close(p.plan().notes[1].start_seconds, 2.5);
        close(p.plan().notes[3].start_seconds, 3.);
        close(p.plan().notes[3].duration_seconds, 0.46);
        close(p.plan().duration_seconds, 3.5);
        for id in ["tie0", "tie1", "tie2"] {
            // Cue-role mapping is not supported by this importer. Exercise the
            // typed SFS role without changing the imported timing or signs.
            let cue = s.replace(&format!("id={id}"), &format!("id={id} kind=cue"));
            let gp = explicit(&cue, &[("g", "1/4")]);
            let fp = holds(&cue, &[(boundary, "1")]);
            assert!(play(&cue, &gp, &fp).unwrap_err().contains("mixed_cue_tie"));
        }
    }
}

fn repeated_xml() -> String {
    let mut imported = include_str!("fixtures/navigation-performed.musicxml")
        .replace("<beats>1</beats>", "<beats>2</beats>");
    for id in ["before", "a", "b", "c", "d"] {
        // This fixture uses divisions=1. A new measured anchor fills its first
        // quarter; the original source note remains its second-quarter principal.
        let anchor = note(
            &format!("held-{id}"),
            1,
            "C",
            "<notations><fermata/></notations>",
        );
        imported = imported.replace(
            &format!("<note id=\"{id}\">"),
            &format!(
                "{anchor}{}<note id=\"{id}\">",
                grace(&format!("grace-{id}"), "")
            ),
        );
    }
    imported
}
fn repeated_source() -> String {
    let original = source(&repeated_xml());
    // Instrument-state import is not supported here; this explicit typed SFS
    // state tests the checkpoint shared with imported tempo/pitch/dynamics.
    at_frame(&original, "4", "= P1 instrument=flute")
}

#[test]
fn repeated_visits_share_route_and_restore_checkpoints_with_distinct_policies() {
    let s = repeated_source();
    let d = discover_source_graces_and_fermatas(&s).unwrap();
    assert_eq!(d.graces.authored.len(), 5);
    assert_eq!(d.fermatas.authored.len(), 5);
    assert_eq!(d.graces.occurrences.len(), 6);
    assert_eq!(d.fermatas.occurrences.len(), 6);
    let gp = ExplicitGracePolicy {
        groups: d
            .graces
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
    let fp = holds(
        &s,
        &[
            ("1", "1"),
            ("3", "1"),
            ("5", "1"),
            ("7", "2"),
            ("9", "1"),
            ("11", "1"),
        ],
    );
    let p = play(&s, &gp, &fp).unwrap();
    assert_eq!(p.plan().notes.len(), 18);
    assert_eq!(
        p.plan()
            .measures
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["prefix", "m1", "m2", "m1", "m3", "m4"]
    );
    for (i, id) in ["before", "a", "b", "a", "c", "d"].into_iter().enumerate() {
        let triple = &p.plan().notes[i * 3..i * 3 + 3];
        assert_eq!(
            triple[0].written_id.as_deref(),
            Some(format!("held-{id}").as_str())
        );
        assert_eq!(
            triple[1].written_id.as_deref(),
            Some(format!("grace-{id}").as_str())
        );
        assert_eq!(triple[2].written_id.as_deref(), Some(id));
        close(triple[0].start_seconds, [0., 3., 6., 8., 12., 15.][i]);
        close(triple[1].start_seconds, [2., 5., 7.5, 11., 14., 17.][i]);
        close(
            triple[1].duration_seconds,
            [0.25, 0.25, 0.125, 0.5, 0.25, 0.25][i],
        );
        close(
            triple[2].start_seconds,
            [2.25, 5.25, 7.625, 11.5, 14.25, 17.25][i],
        );
        close(triple[1].midi_note, [72., 72., 71., 72., 72., 72.][i]);
        close(triple[2].midi_note, [57., 58., 59., 58., 62., 63.][i]);
        for n in triple {
            assert_eq!(n.velocity, [45, 45, 90, 45, 45, 45][i]);
            assert_eq!(n.instrument, if i == 2 { "flute" } else { "piano" });
        }
    }
    assert_ne!(p.plan().notes[3].id, p.plan().notes[9].id);
    assert_ne!(p.plan().notes[4].id, p.plan().notes[10].id);
    assert_ne!(p.plan().notes[5].id, p.plan().notes[11].id);
    close(p.plan().duration_seconds, 18.);
    assert_eq!(p.timing().duration_seconds(), &r("18"));
    assert_eq!(p.timing().quarter_end(), &r("12"));
    assert_eq!(p.timing().holds()[3].added_seconds, r("2"));
    assert_eq!(
        p.grace_groups()
            .iter()
            .find(|g| g.principal == d.graces.occurrences[3].principal)
            .unwrap()
            .allocated_quarters,
        r("1/2")
    );
    assert_eq!(
        d.graces
            .authored
            .iter()
            .find(|a| a.written_id.0 == "grace-a")
            .unwrap()
            .occurrence_ids
            .len(),
        2
    );
    assert_eq!(
        d.fermatas
            .authored
            .iter()
            .find(|a| a.sign.event.0 == "held-a")
            .unwrap()
            .occurrence_ids
            .len(),
        2
    );
    assert_eq!(
        d.graces
            .authored
            .iter()
            .find(|a| a.written_id.0 == "grace-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1
    );
    let mut wrong = gp.clone();
    wrong.groups[1].principal = EventId("a".into());
    reject(&s, &wrong, &fp, "grace.policy_principal");
    let mut missing = fp.clone();
    missing.groups.remove(3);
    reject(&s, &gp, &missing, "fermata.");
}

#[test]
fn skipped_authored_signs_stay_in_both_audits_without_invented_policies() {
    // The first ending's signs are skipped on the second pass, not erased from
    // the authored audit or duplicated into a nonexistent performed occurrence.
    // The supported closed ordinary-repeat profile visits each written ending
    // at least once; no unsupported navigation is invented to force zero visits.
    let s = repeated_source();
    let d = discover_source_graces_and_fermatas(&s).unwrap();
    assert_eq!(d.graces.authored.len(), 5);
    assert_eq!(d.fermatas.authored.len(), 5);
    assert_eq!(
        d.graces
            .authored
            .iter()
            .find(|a| a.written_id.0 == "grace-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1
    );
    assert_eq!(
        d.fermatas
            .authored
            .iter()
            .find(|a| a.sign.event.0 == "held-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1
    );
    let gp = ExplicitGracePolicy {
        groups: d
            .graces
            .occurrences
            .iter()
            .map(|o| ExplicitGraceGroup {
                principal: o.principal.clone(),
                allocation: GraceAllocation::ExplicitFollowingQuarters {
                    members: vec![GraceQuarterAllocation {
                        event: o.id.clone(),
                        quarters: r("1/4"),
                    }],
                },
            })
            .collect(),
    };
    let fp = ExplicitFermataPolicy {
        groups: d
            .fermatas
            .occurrences
            .iter()
            .enumerate()
            .map(|(i, o)| ExplicitHoldGroup {
                id: format!("visited-{i}"),
                members: vec![o.id.clone()],
                added_seconds: r("1"),
            })
            .collect(),
    };
    let p = play(&s, &gp, &fp).unwrap();
    assert_eq!(
        p.plan()
            .notes
            .iter()
            .filter(|n| n.written_id.as_deref() == Some("b"))
            .count(),
        1
    );
    assert_eq!(
        p.plan()
            .notes
            .iter()
            .filter(|n| n.written_id.as_deref() == Some("a"))
            .count(),
        2
    );
    assert_eq!(
        p.grace_discovery()
            .authored
            .iter()
            .find(|a| a.written_id.0 == "grace-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1
    );
    assert_eq!(
        p.timing()
            .authored
            .iter()
            .find(|a| a.sign.event.0 == "held-b")
            .unwrap()
            .occurrence_ids
            .len(),
        1
    );
}

#[test]
fn unselected_sounding_overlap_and_selection_at_a_different_hold_reject() {
    for ticks in [8, 16] {
        let first = note("held", 8, "C", "<notations><fermata/></notations>")
            + &grace("g", "")
            + &note("main", 8, "G", "");
        let other = note("unselected", ticks, "A", "")
            + &if ticks == 8 {
                note("tail", 8, "rest", "")
            } else {
                String::new()
            };
        let s = source(&xml(&[first, other], 2));
        reject(
            &s,
            &explicit(&s, &[("g", "1/4")]),
            &holds(&s, &[("1", "1")]),
            "fermata.unselected_overlap",
        );
    }
    // The tie is selected at q1, not globally exempt from another group's q2.
    let s = tied_source(0);
    // The MusicXML source form is kept for this test, including the original tie.
    let model = SfsAdapter.read(&s).unwrap().value;
    let imported = MusicXml.write(&model).unwrap().value;
    let added = note("other-lead", 8, "rest", "")
        + &note(
            "other-anchor",
            8,
            "rest",
            "<notations><fermata/></notations>",
        )
        + &note("other-tail", 24, "rest", "");
    let imported = imported.replace("</part-list>", "<score-part id=\"extra\"><part-name>Other</part-name><score-instrument id=\"extra-i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list>")
        .replace("</score-partwise>", &format!("<part id=\"extra\"><measure id=\"extra-m\" number=\"1\"><attributes><divisions>8</divisions><time><beats>5</beats><beat-type>4</beat-type></time></attributes>{added}</measure></part></score-partwise>"));
    let s = source(&imported);
    reject(
        &s,
        &explicit(&s, &[("g", "1/4")]),
        &holds(&s, &[("1", "1"), ("2", "1")]),
        "fermata.unselected_overlap",
    );
}

#[test]
fn exact_count_rational_and_combined_duration_limits_are_not_weakened() {
    let s = single("", "");
    let gp = explicit(&s, &[("g", "1/8")]);
    let fp = holds(&s, &[("1", "1")]);
    let doc = scorestream_codec::parse(&s).unwrap();
    let qualify = |grace, timing, performance| {
        perform_document_with_explicit_graces_and_fermatas(
            &doc,
            &gp,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &fp,
            performance,
            grace,
            timing,
        )
    };
    assert!(
        qualify(
            GraceTimingLimits::default(),
            TimingLimits::default(),
            PerformanceLimits::default()
        )
        .is_ok()
    );
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
            maximum_timeline_seconds: Some(r("3/2")),
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_timeline_seconds: Some(r("0")),
            ..Default::default()
        },
    ] {
        assert!(
            qualify(
                limits,
                TimingLimits::default(),
                PerformanceLimits::default()
            )
            .is_err()
        );
    }
    for limits in [
        TimingLimits {
            maximum_source_records: 1,
            ..Default::default()
        },
        TimingLimits {
            maximum_events: 1,
            ..Default::default()
        },
        TimingLimits {
            maximum_measures: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_authored_fermatas: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_tempos: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_hold_groups: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_group_members: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_rational_bits: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_timeline_seconds: Some(r("3/2")),
            ..Default::default()
        },
        TimingLimits {
            maximum_timeline_seconds: Some(r("0")),
            ..Default::default()
        },
    ] {
        assert!(
            qualify(
                GraceTimingLimits::default(),
                limits,
                PerformanceLimits::default()
            )
            .is_err()
        );
    }
    assert!(
        qualify(
            GraceTimingLimits::default(),
            TimingLimits::default(),
            PerformanceLimits {
                maximum_occurrences: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    // The base timeline fits each limit, but its total including policy holds
    // does not. The stricter bound from either profile applies to the same clock.
    for limits in [r("199/100"), r("1")] {
        assert!(
            qualify(
                GraceTimingLimits {
                    maximum_timeline_seconds: Some(limits.clone()),
                    ..Default::default()
                },
                TimingLimits::default(),
                PerformanceLimits::default()
            )
            .is_err()
        );
        assert!(
            qualify(
                GraceTimingLimits::default(),
                TimingLimits {
                    maximum_timeline_seconds: Some(limits),
                    ..Default::default()
                },
                PerformanceLimits::default()
            )
            .is_err()
        );
    }
    let long = plan_source_with_explicit_graces_and_fermatas(
        &s,
        &gp,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &holds(&s, &[("1", "21600")]),
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap();
    assert!(long.plan().duration_seconds > 21600.0);
}

#[test]
fn final_shifted_windows_enforce_precision_and_rational_budgets() {
    let s = single("", "");
    for amount in ["1/10000000", "9999999999/10000000000"] {
        reject(
            &s,
            &explicit(&s, &[("g", amount)]),
            &holds(&s, &[("1", "10000")]),
            "grace.clock_precision",
        );
    }
    // Base 0.500001 - 0.5 is representable above one microsecond, but the
    // final shifted 32.500001 - 32.5 rounds below the supported audio minimum.
    reject(
        &s,
        &explicit(&s, &[("g", "1/500000")]),
        &holds(&s, &[("1", "32")]),
        "combined.clock_precision",
    );
    let doc = scorestream_codec::parse(&s).unwrap();
    let gp = explicit(&s, &[("g", "1/8")]);
    let fp = holds(&s, &[("1", "10000")]);
    let errors = perform_document_with_explicit_graces_and_fermatas(
        &doc,
        &gp,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &fp,
        PerformanceLimits::default(),
        GraceTimingLimits {
            maximum_rational_bits: 16,
            ..Default::default()
        },
        TimingLimits::default(),
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|d| d.code == "combined.rational_budget"),
        "{errors:?}"
    );
    let p = play(&s, &gp, &fp).unwrap();
    assert_eq!(p.grace_windows()[0].seconds_start, r("20001/2"));
    assert_eq!(p.timing().duration_seconds(), &r("10001"));
    reject(
        &s,
        &gp,
        &holds(&s, &[("1", "1/1000000000000000000000000000000")]),
        "fermata.",
    );
}

#[test]
fn desktop_fixture_is_generated_only_from_the_actual_qualified_joint_plan() {
    let s = source(XML);
    let p = play(
        &s,
        &explicit(&s, &[("z-grace", "1/8"), ("a-grace", "1/4")]),
        &holds(&s, &[("1", "1")]),
    )
    .unwrap();
    assert_eq!(p.plan().notes.len(), 4);
    close(p.plan().duration_seconds, 21. / 8.);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/tests/fixtures/joint-playback.json");
    let actual = serde_json::to_value(p.plan()).unwrap();
    if std::env::var_os("UPDATE_JOINT_FIXTURE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let checked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        checked, actual,
        "Regenerate only from unchanged synthetic XML with UPDATE_JOINT_FIXTURE=1"
    );
}

#[test]
fn combined_clock_cannot_silently_ignore_initial_midi_intent() {
    let plain = source(XML);
    let grace = explicit(&plain, &[("z-grace", "1/8"), ("a-grace", "1/4")]);
    let fermata = holds(&plain, &[("1", "1")]);
    assert!(play(&plain, &grace, &fermata).is_ok());
    let mark = plain
        .lines()
        .find(|l| l.contains(" scorestream.instrument "))
        .unwrap();
    // Empty device is still authored intent, not permission to choose an output.
    let assigned = plain.replace(mark, &format!("{mark} device=\"\""));
    assert!(SfsAdapter.read(&assigned).is_ok());
    reject(
        &assigned,
        &grace,
        &fermata,
        "instrument.performance_unsupported",
    );
}

fn synchronized_holds(
    source: &str,
    boundary: &str,
) -> scorestream_core::fermata_timing::ExplicitSynchronizedFermataPolicy {
    use scorestream_core::fermata_timing::{
        ExplicitSynchronizedFermataPolicy, ExplicitSynchronizedHoldGroup,
    };
    ExplicitSynchronizedFermataPolicy {
        groups: vec![ExplicitSynchronizedHoldGroup {
            hold: ExplicitHoldGroup {
                id: "explicit-sync".into(),
                members: discover_source_graces_and_fermatas(source)
                    .unwrap()
                    .fermatas
                    .occurrences
                    .iter()
                    .map(|o| o.id.clone())
                    .collect(),
                added_seconds: r("3/2"),
            },
            boundary: r(boundary),
        }],
    }
}
fn play_synchronized(source: &str, boundary: &str) -> Result<ExplicitCombinedPlayback, String> {
    scorestream_app::playback::plan_source_with_explicit_graces_and_synchronized_fermatas(
        source,
        &explicit(source, &[("g", "1/4")]),
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &synchronized_holds(source, boundary),
        FermataSoundPolicy::SustainSelectedAnchors,
    )
}
#[test]
fn synchronized_short_rest_and_later_grace_use_one_qualified_clock() {
    use scorestream_core::fermata_timing::HoldSynchronization;
    let s = source(include_str!(
        "fixtures/synchronized-joint-playback.musicxml"
    ));
    let before = scorestream_codec::parse(&s).unwrap();
    let p = play_synchronized(&s, "1").unwrap();
    assert_eq!(scorestream_codec::parse(&s).unwrap(), before);
    assert_eq!(p.timing().holds().len(), 1);
    let hold = &p.timing().holds()[0];
    assert_eq!(
        hold.synchronization,
        HoldSynchronization::ExplicitLatestReleaseWithEarlierRests
    );
    assert_eq!(
        (&hold.boundary, &hold.seconds_start, &hold.seconds_end),
        (&r("1"), &r("1/2"), &r("2"))
    );
    assert_eq!(p.grace_windows().len(), 2);
    assert_eq!(p.grace_windows()[0].seconds_start, r("2"));
    assert_eq!(p.grace_windows()[0].seconds_end, r("17/8"));
    assert_eq!(p.grace_windows()[1].seconds_start, r("17/8"));
    assert_eq!(p.grace_windows()[1].seconds_end, r("247/100"));
    assert_eq!(p.plan().notes.len(), 3);
    close(p.plan().notes[0].duration_seconds, 2.0);
    close(p.plan().notes[1].start_seconds, 2.0);
    close(p.plan().notes[1].duration_seconds, 0.125);
    close(p.plan().notes[2].start_seconds, 2.125);
    close(p.plan().duration_seconds, 2.5);
    let short = p
        .plan()
        .event_positions
        .iter()
        .find(|m| m.id == "short-rest")
        .unwrap();
    close(short.end_seconds, 0.25);
    assert_eq!(
        p.timing()
            .fermatas
            .iter()
            .find(|f| f.written_id.0 == "short-rest")
            .unwrap()
            .written_quarter_end,
        r("1/2")
    );
    assert!(matches!(p.seek(&r("3/2")), SeekPosition::Hold { boundary, .. } if boundary == r("1")));
    let ordinary = ExplicitFermataPolicy {
        groups: synchronized_holds(&s, "1")
            .groups
            .into_iter()
            .map(|g| g.hold)
            .collect(),
    };
    assert!(
        play(&s, &explicit(&s, &[("g", "1/4")]), &ordinary)
            .unwrap_err()
            .contains("unequal_boundaries")
    );
    assert!(
        play_synchronized(&s, "1/2")
            .unwrap_err()
            .contains("synchronization_boundary")
    );
}
#[test]
fn synchronized_combined_keeps_common_release_and_overlap_contracts() {
    let s = single("", "");
    let old = play(
        &s,
        &explicit(&s, &[("g", "1/4")]),
        &holds(&s, &[("1", "3/2")]),
    )
    .unwrap();
    let new = play_synchronized(&s, "1").unwrap();
    assert_eq!(old.plan().notes, new.plan().notes);
    assert_eq!(old.plan().event_positions, new.plan().event_positions);
    assert_eq!(old.grace_windows(), new.grace_windows());
    let overlapping = source(&xml(
        &[
            format!("{}{}", grace("g", ""), note("principal", 16, "C", "")),
            format!(
                "{}{}",
                note("short", 4, "rest", "<notations><fermata/></notations>"),
                note("tail1", 12, "rest", "")
            ),
            format!(
                "{}{}",
                note("long", 8, "rest", "<notations><fermata/></notations>"),
                note("tail2", 8, "rest", "")
            ),
        ],
        2,
    ));
    assert!(
        play_synchronized(&overlapping, "1")
            .unwrap_err()
            .contains("combined.grace_hold_overlap")
    );
}
