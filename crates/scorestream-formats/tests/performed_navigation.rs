use scorestream_core::{Rational, notation::*};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
const XML: &str = include_str!("fixtures/navigation-performed.musicxml");
fn sfs(xml: &str) -> String {
    SfsAdapter
        .write(&MusicXml.read(xml).unwrap().value)
        .unwrap()
        .value
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn xml_to_sfs_to_playback_replays_with_checkpoints_and_written_identity() {
    let source = sfs(XML);
    let doc = scorestream_codec::parse(&source).unwrap();
    let before = doc.clone();
    let performed = perform_document(&doc, PerformanceLimits::default()).unwrap();
    assert_eq!(doc, before);
    assert_eq!(doc.event_count(), 5);
    assert_eq!(performed.score.events.len(), 6);
    assert_eq!(
        performed
            .events
            .iter()
            .map(|e| e.written_id.0.as_str())
            .collect::<Vec<_>>(),
        ["before", "a", "b", "a", "c", "d"]
    );
    assert_eq!(performed.score.end, "6".parse::<Rational>().unwrap());
    assert_eq!(
        performed
            .score
            .events
            .iter()
            .map(|e| e.sounding_pitch.as_ref().unwrap().to_string())
            .collect::<Vec<_>>(),
        ["45", "46", "47", "46", "50", "51"]
    );
    let p = scorestream_app::playback::plan_source(&source).unwrap();
    close(p.duration_seconds, 5.5);
    assert_eq!(
        p.notes
            .iter()
            .map(|n| n.written_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["before", "a", "b", "a", "c", "d"]
    );
    assert_eq!(
        p.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [45, 45, 90, 45, 45, 45]
    );
    for (n, t) in p.notes.iter().zip([0., 1., 2., 2.5, 3.5, 4.5]) {
        close(n.start_seconds, t);
    }
    assert_eq!(
        p.event_positions
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["before", "a", "b", "a", "c", "d"]
    );
    assert_ne!(
        p.event_positions[1].occurrence_id,
        p.event_positions[3].occurrence_id
    );
    assert_eq!(
        p.measures.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["prefix", "m1", "m2", "m1", "m3", "m4"]
    );
    assert!(
        !p.diagnostics
            .iter()
            .any(|d| d.code == "navigation.conventional_two_passes"),
        "Explicit ending pass numbers need no count inference"
    );
    assert_eq!(source, sfs(XML));
}
#[test]
fn open_ending_and_projection_budgets_reject_without_fallback() {
    let source = sfs(XML);
    let doc = scorestream_codec::parse(&source).unwrap();
    for limits in [
        PerformanceLimits {
            maximum_occurrences: 5,
            ..Default::default()
        },
        PerformanceLimits {
            maximum_events: 5,
            ..Default::default()
        },
        PerformanceLimits {
            maximum_controls: 1,
            ..Default::default()
        },
    ] {
        assert!(perform_document(&doc, limits).is_err());
    }
    let open = source
        .lines()
        .filter(|l| !l.contains("ending=discontinue"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        scorestream_app::playback::plan_source(&open)
            .unwrap_err()
            .contains("UnclosedFinalEnding")
    );
}
#[test]
fn ordinary_no_navigation_playback_keeps_legacy_ids() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=a\n.\n!end at=1\n";
    let p = scorestream_app::playback::plan_source(source).unwrap();
    assert_eq!(p.notes[0].id, "a");
    assert_eq!(p.notes[0].written_id, None);
    assert_eq!(p.event_positions[0].id, "a");
    assert_eq!(p.event_positions[0].occurrence_id, None);
    assert_eq!(p.measures[0].occurrence_index, None);
    close(p.duration_seconds, 0.5);
}

fn wire(frames: &str, end: usize, staves: usize) -> String {
    format!(
        "!score 1.1 unit=q\n!requires scorestream.navigation@1\n!part p instrument=piano staves={staves}\n!voice v part=p staff=1\n{frames}\n!end at={end}\n"
    )
}
#[test]
fn scoped_state_mid_measure_tempo_and_staff_override_restore_without_leakage() {
    let source = wire(
        r#"@0
| p m0 len=1
= score tempo=q:60
= p sound_dynamics=50 transpose_chromatic=-2 transpose_diatonic=-1 transpose_octaves=0
= p.s2 sound_dynamics=80 transpose_chromatic=12 transpose_diatonic=7 transpose_octaves=0
v C4:1 id=prefix
.
@1
| p m1 len=2
+ p scorestream.barline measure=m1 side=left repeat=forward
v C4:2 id=held
.
@2
= score tempo=q:120
.
@3
| p m2 len=1
+ p scorestream.barline measure=m2 side=left ending=start passes="1"
= v staff=2
= p instrument=flute
= p.s1 sound_dynamics=100
= p.s2 sound_dynamics=120 transpose_chromatic=-12 transpose_diatonic=-7 transpose_octaves=0
v E4:1 id=first
.
@4
+ p scorestream.barline measure=m2 side=right repeat=backward ending=stop passes="1"
| p m3 len=1
+ p scorestream.barline measure=m3 side=left ending=start passes="2"
v F4:1 id=second
.
@5
+ p scorestream.barline measure=m3 side=right ending=discontinue passes="2"
| p m4 len=1
v G4:1 id=tail
."#,
        6,
        2,
    );
    let doc = scorestream_codec::parse(&source).unwrap();
    let performed = perform_document(&doc, PerformanceLimits::default()).unwrap();
    assert_eq!(
        performed
            .score
            .events
            .iter()
            .map(|e| (
                e.origin.object_id.as_str(),
                e.staff.number,
                e.instrument.0.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("prefix", 1, "piano"),
            ("held", 1, "piano"),
            ("first", 2, "flute"),
            ("held", 1, "piano"),
            ("second", 1, "piano"),
            ("tail", 1, "piano")
        ]
    );
    let p = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(
        p.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [45, 45, 108, 45, 45, 45]
    );
    assert_eq!(
        p.notes.iter().map(|n| n.midi_note).collect::<Vec<_>>(),
        [58., 58., 52., 58., 63., 65.]
    );
    for (n, t) in p.notes.iter().zip([0., 1., 2.5, 3., 4.5, 5.]) {
        close(n.start_seconds, t);
    }
    close(p.duration_seconds, 5.5);
    // The inherited 60 BPM is restored, then the interior 120 BPM is replayed.
    close(p.notes[1].duration_seconds, 1.42);
    close(p.notes[3].duration_seconds, 1.42);
}
#[test]
fn repeated_contiguous_ties_rebind_and_cross_jump_ties_reject() {
    let good = wire(
        r#"@0
| p m1 len=1
+ p scorestream.barline measure=m1 side=left repeat=forward
v C4:1 id=a
~ tie t start=#a
.
@1
| p m2 len=1
v C4:1 id=b
~ tie t end=#b
+ #b articulation value=tenuto
.
@2
+ p scorestream.barline measure=m2 side=right repeat=backward times=2
| p m3 len=1
v D4:1 id=c
."#,
        3,
        1,
    );
    let p = scorestream_app::playback::plan_source(&good).unwrap();
    assert_eq!(p.notes.len(), 3);
    assert_eq!(p.event_positions.len(), 5);
    close(p.notes[0].duration_seconds, 1.);
    close(p.notes[1].start_seconds, 1.);
    close(p.notes[1].duration_seconds, 1.);
    assert_ne!(p.notes[0].id, p.notes[1].id);
    // A tie from the repeating tail into the post-repeat measure has no first-pass
    // endpoint, so neither silent reattack nor a long jump-spanning sustain is allowed.
    let bad = good
        .replace("~ tie t start=#a\n", "")
        .replace("~ tie t end=#b", "~ tie t start=#b")
        .replace("v D4:1 id=c", "v C4:1 id=c\n~ tie t end=#c");
    assert!(
        scorestream_app::playback::plan_source(&bad)
            .unwrap_err()
            .contains("navigation.relation_jump")
    );
    let span = good
        .replace("~ tie t start=#a", "~ slur t start=#a")
        .replace("~ tie t end=#b", "~ slur t end=#b");
    assert!(
        scorestream_app::playback::plan_source(&span)
            .unwrap_err()
            .contains("navigation.span_unsupported")
    );
}
#[test]
fn reset_absent_tempo_and_dynamics_to_defaults_and_report_implicit_policy() {
    let source = wire(
        r#"@0
| p m1 len=1
v C4:1 id=a
.
@1
| p m2 len=1
= score tempo=q:60
= p sound_dynamics=100
v D4:1 id=b
.
@2
+ p scorestream.barline measure=m2 side=right repeat=backward
| p m3 len=1
v E4:1 id=c
."#,
        3,
        1,
    );
    let p = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(
        p.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [80, 90, 80, 90, 90]
    );
    for (n, t) in p.notes.iter().zip([0., 0.5, 1.5, 2., 3.]) {
        close(n.start_seconds, t);
    }
    close(p.duration_seconds, 4.);
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == "navigation.conventional_two_passes")
    );
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == "navigation.implicit_start")
    );
    let symbolic = source.replace("= p sound_dynamics=100", "+ p dynamic value=f");
    assert!(
        scorestream_app::playback::plan_source(&symbolic)
            .unwrap_err()
            .contains("navigation.symbolic_dynamics")
    );
    let eof = source.replace("!end at=3", "@3\n= score tempo=q:60\n.\n!end at=3");
    assert!(
        scorestream_app::playback::plan_source(&eof)
            .unwrap_err()
            .contains("navigation.final_boundary_control")
    );
}

// Regression found by independent review: written equality is not performed equality.
fn skip_tie() -> String {
    wire(
        r#"@0
| p m0 len=1
+ p scorestream.barline measure=m0 side=left repeat=forward
v C4:1 id=a
.
@1
| p m1 len=1
+ p scorestream.barline measure=m1 side=left ending=start passes="1"
= p transpose_chromatic=12 transpose_diatonic=7 transpose_octaves=0
v D4:1 id=x
.
@2
+ p scorestream.barline measure=m1 side=right ending=stop passes="1" repeat=backward
| p m2 len=1
+ p scorestream.barline measure=m2 side=left ending=start passes="2"
v C4:1 id=b
~ tie t start=#b
.
@3
+ p scorestream.barline measure=m2 side=right ending=discontinue passes="2"
| p m3 len=1
= p transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=0
v C5:1 id=c
~ tie t end=#c
."#,
        4,
        1,
    )
}
#[test]
fn independent_tie_pitch_is_revalidated_after_skipped_state() {
    let source = skip_tie();
    let doc = scorestream_codec::parse(&source).expect("written tie is C5 to C5");
    let performed = perform_document(&doc, PerformanceLimits::default());
    assert!(
        performed
            .unwrap_err()
            .iter()
            .any(|d| d.code == "navigation.tie_pitch")
    );
    let plan = scorestream_app::playback::plan_source(&source);
    assert!(
        plan.is_err(),
        "After skipping the +12 first-ending control, b sounds C4 but c sounds C5. Must reject instead of suppressing c: {plan:?}"
    );
}
#[test]
fn independent_tie_survives_when_both_projected_pitches_change_equally() {
    let source = skip_tie()
        .replace(
            "= p transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=0\n",
            "",
        )
        .replace("v C5:1 id=c", "v C4:1 id=c");
    let plan = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(plan.notes.last().unwrap().written_id.as_deref(), Some("b"));
    assert_eq!(plan.notes.last().unwrap().midi_note, 60.);
    assert_eq!(plan.event_positions.len(), 5);
}

#[test]
fn combined_fermata_and_navigation_never_bypass_hold_policy() {
    let xml = XML.replacen(
        "</type></note>",
        "</type><notations><fermata type=\"upright\"/></notations></note>",
        1,
    );
    let source = sfs(&xml);
    let doc = scorestream_codec::parse(&source).unwrap();
    assert!(
        doc.required_extensions
            .iter()
            .any(|e| e == scorestream_core::fermata_extension::EXTENSION)
    );
    assert!(
        doc.required_extensions
            .iter()
            .any(|e| e == scorestream_core::navigation_extension::EXTENSION)
    );
    assert!(
        perform_document(&doc, PerformanceLimits::default())
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
    assert!(
        scorestream_app::playback::plan_source(&source)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    assert!(
        lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
}
#[test]
fn display_only_and_unpitched_ties_preserve_ordinary_contracts() {
    let printed = skip_tie().replace("~ tie t start=#b", "~ tie t start=#b sound=false");
    let p = scorestream_app::playback::plan_source(&printed).unwrap();
    assert_eq!(
        p.notes.len(),
        5,
        "Display-only ties must not suppress attacks or require equal sounding pitch"
    );
    let drums = wire(
        r#"@0
| p m0 len=1
+ p scorestream.barline measure=m0 side=left repeat=forward
v u.snare:1 id=a
~ tie t start=#a
.
@1
| p m1 len=1
v u.snare:1 id=b
~ tie t end=#b
.
@2
+ p scorestream.barline measure=m1 side=right repeat=backward times=2
."#,
        2,
        1,
    );
    let doc = scorestream_codec::parse(&drums).unwrap();
    assert_eq!(
        perform_document(&doc, PerformanceLimits::default())
            .unwrap()
            .score
            .relations
            .len(),
        2
    );
}
