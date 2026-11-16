//! Synthetic MusicXML → shared model → unchanged SFS → qualified audio contracts.
//! These are shape analogues, not transcriptions or repaired Beethoven sources.
use scorestream_app::playback::{
    ExplicitFermataPlayback, FermataSoundPolicy, discover_source_fermatas, plan_source,
    plan_source_with_explicit_fermatas,
};
use scorestream_core::{
    Rational,
    fermata_timing::{ExplicitFermataPolicy, ExplicitHoldGroup, SeekPosition},
    notation::{PerformanceLimits, lower_document, perform_document},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn note(id: &str, ticks: usize, rest: bool, mark: bool) -> String {
    let pitch = if rest {
        "<rest/>"
    } else {
        "<pitch><step>G</step><octave>4</octave></pitch>"
    };
    let notation = if mark {
        "<notations><fermata type=\"upright\">normal</fermata></notations>"
    } else {
        ""
    };
    format!(
        "<note id=\"{id}\">{pitch}<duration>{ticks}</duration><voice>1</voice>{notation}</note>"
    )
}
fn xml(parts: &[String], quarters: usize) -> String {
    let list = (0..parts.len()).map(|i| format!("<score-part id=\"p{i}\"><part-name>Piano</part-name><score-instrument id=\"i{i}\"><instrument-name>piano</instrument-name></score-instrument></score-part>")).collect::<String>();
    let bodies = parts.iter().enumerate().map(|(i, notes)| format!("<part id=\"p{i}\"><measure id=\"m{i}\" number=\"1\"><attributes><divisions>2</divisions><time><beats>{quarters}</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part>")).collect::<String>();
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
    let doc = scorestream_codec::parse(&source).unwrap();
    assert!(
        lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
    assert!(
        perform_document(&doc, PerformanceLimits::default())
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
    assert!(
        plan_source(&source)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    source
}
// The test explicitly chooses each group and seconds; discovery supplies identities.
fn policy(source: &str, boundaries: &[(&str, &str)]) -> ExplicitFermataPolicy {
    let discovered = discover_source_fermatas(source).unwrap();
    ExplicitFermataPolicy {
        groups: boundaries
            .iter()
            .enumerate()
            .map(|(i, (q, seconds))| ExplicitHoldGroup {
                id: format!("explicit-{i}"),
                members: discovered
                    .occurrences
                    .iter()
                    .filter(|o| o.quarter_end == r(q))
                    .map(|o| o.id.clone())
                    .collect(),
                added_seconds: r(seconds),
            })
            .collect(),
    }
}
fn play(source: &str, policy: &ExplicitFermataPolicy) -> Result<ExplicitFermataPlayback, String> {
    let original = source.to_owned();
    let document = scorestream_codec::parse(source).unwrap();
    let result = plan_source_with_explicit_fermatas(
        source,
        policy,
        FermataSoundPolicy::SustainSelectedAnchors,
    );
    assert_eq!(source, original);
    assert_eq!(scorestream_codec::parse(source).unwrap(), document);
    result
}
fn intervals(anchors: &[(usize, usize, bool)], quarters: usize) -> String {
    let parts = anchors
        .iter()
        .enumerate()
        .map(|(i, (start, end, rest))| {
            let mut body = String::new();
            if *start > 0 {
                body += &note(&format!("lead{i}"), *start, true, false);
            }
            body += &note(&format!("anchor{i}"), end - start, *rest, true);
            if *end < quarters * 2 {
                body += &note(&format!("tail{i}"), quarters * 2 - end, true, false);
            }
            body
        })
        .collect::<Vec<_>>();
    source(&xml(&parts, quarters))
}

#[test]
fn eighteen_authored_copies_are_one_explicit_global_hold() {
    let s = intervals(&vec![(0, 4, false); 18], 2);
    let d = discover_source_fermatas(&s).unwrap();
    assert_eq!(d.authored.len(), 18);
    assert_eq!(d.occurrences.len(), 18);
    let p = play(&s, &policy(&s, &[("2", "3/2")])).unwrap();
    assert_eq!(p.timing.duration_seconds(), &r("5/2"));
    close(p.plan.duration_seconds, 2.5);
    assert_eq!(p.plan.notes.len(), 18);
    for n in &p.plan.notes {
        close(n.duration_seconds, 2.5);
    }
}
#[test]
fn i21_unequal_note_and_rest_starts_share_one_release() {
    let mut anchors = vec![(0, 4, false)];
    anchors.extend(vec![(2, 4, true); 16]);
    let s = intervals(&anchors, 2);
    let p = play(&s, &policy(&s, &[("2", "1")])).unwrap();
    assert_eq!(p.timing.fermatas.len(), 17);
    assert_eq!(p.plan.notes.len(), 1);
    close(p.plan.notes[0].duration_seconds, 2.);
    assert!(p.timing.fermatas.iter().any(|a| a.quarter_start == r("1")));
}
#[test]
fn iv446_shorter_final_anchor_shares_the_common_end() {
    let mut anchors = vec![(0, 8, false); 18];
    anchors.push((4, 8, false));
    let s = intervals(&anchors, 4);
    let p = play(&s, &policy(&s, &[("4", "1")])).unwrap();
    close(p.plan.duration_seconds, 3.);
    assert_eq!(p.plan.notes.len(), 19);
    for n in &p.plan.notes {
        close(n.start_seconds + n.duration_seconds, 3.);
    }
}
#[test]
fn ii123_unequal_releases_and_overlapping_split_groups_reject() {
    let s = intervals(&[(0, 2, false), (0, 1, true)], 1);
    let d = discover_source_fermatas(&s).unwrap();
    let all = ExplicitFermataPolicy {
        groups: vec![ExplicitHoldGroup {
            id: "all".into(),
            members: d.occurrences.iter().map(|o| o.id.clone()).collect(),
            added_seconds: r("1"),
        }],
    };
    assert!(play(&s, &all).is_err());
    assert!(play(&s, &policy(&s, &[("1/2", "1"), ("1", "1")])).is_err());
}
#[test]
fn i268_disjoint_anchors_have_three_deliberately_chosen_holds() {
    let s = intervals(&[(0, 4, true), (10, 12, true), (18, 20, true)], 10);
    let p = play(&s, &policy(&s, &[("2", "1/3"), ("6", "2/3"), ("10", "1")])).unwrap();
    assert_eq!(p.timing.duration_seconds(), &r("7"));
    close(p.plan.duration_seconds, 7.);
    assert!(p.plan.notes.is_empty());
}
#[test]
fn no_omission_duplicate_membership_or_float_collapsed_hold_is_accepted() {
    let s = intervals(&[(0, 2, false)], 1);
    assert!(play(&s, &ExplicitFermataPolicy { groups: vec![] }).is_err());
    let mut p = policy(&s, &[("1", "1")]);
    let member = p.groups[0].members[0].clone();
    p.groups[0].members.push(member);
    assert!(play(&s, &p).is_err());
    assert!(
        play(
            &s,
            &policy(&s, &[("1", "1/1000000000000000000000000000000")])
        )
        .is_err()
    );
}
fn tied_body(mark: usize, numeric: bool) -> String {
    let mut body = if numeric {
        "<sound dynamics=\"50\"/>".to_owned()
    } else {
        String::new()
    };
    for i in 0..3 {
        if numeric && i == 1 {
            body += "<sound dynamics=\"100\"/>";
        }
        let ties = match i {
            0 => "<tie type=\"start\"/>",
            1 => "<tie type=\"stop\"/><tie type=\"start\"/>",
            _ => "<tie type=\"stop\"/>",
        };
        let notation_ties = ties.replace("<tie ", "<tied ");
        let fermata = if i == mark { "<fermata/>" } else { "" };
        body += &format!(
            "<note id=\"tie{i}\"><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration>{ties}<voice>1</voice><notations>{notation_ties}{fermata}</notations></note>"
        );
    }
    body
}
fn tied(mark: usize, numeric: bool) -> String {
    source(&xml(&[tied_body(mark, numeric)], 3))
}
#[test]
fn terminal_and_interior_tie_anchors_preserve_one_attack_and_own_markers() {
    for (mark, boundary) in [(2, "3"), (0, "1"), (1, "2")] {
        let s = tied(mark, false);
        let p = play(&s, &policy(&s, &[(boundary, "1")])).unwrap();
        assert_eq!(p.plan.notes.len(), 1);
        assert_eq!(p.plan.event_positions.len(), 3);
        assert_eq!(p.timing.fermatas[0].written_id.0, format!("tie{mark}"));
        assert!(p.plan.notes[0].duration_seconds > 2.);
        if mark == 2 {
            close(p.plan.notes[0].duration_seconds, 2.5);
        }
    }
}
#[test]
fn unmarked_sounding_overlap_or_common_end_rejects_but_boundary_attack_is_allowed() {
    for ticks in [2, 4] {
        let s = source(&xml(
            &[
                note("selected", 2, false, true) + &note("rest", 2, true, false),
                note("unmarked", ticks, false, false)
                    + &if ticks == 2 {
                        note("rest2", 2, true, false)
                    } else {
                        String::new()
                    },
            ],
            2,
        ));
        assert!(play(&s, &policy(&s, &[("1", "1")])).is_err());
    }
    let s = source(&xml(
        &[note("selected", 2, false, true) + &note("next", 2, false, false)],
        2,
    ));
    let p = play(&s, &policy(&s, &[("1", "1")])).unwrap();
    close(p.plan.notes[1].start_seconds, 1.5);
}
#[test]
fn selected_out_of_range_pitch_rejects_qualified_playback() {
    let body = note("selected", 2, false, true)
        .replace("<octave>4</octave>", "<octave>9</octave>")
        .replace("<step>G</step>", "<step>B</step>");
    let s = source(&xml(&[body], 1));
    let error = play(&s, &policy(&s, &[("1", "1")])).unwrap_err();
    assert!(
        error.contains("fermata.unsupported_sound_anchor"),
        "{error}"
    );
}
#[test]
fn cue_and_unpitched_fail_import_but_articulation_rejects_at_sound_policy_boundary() {
    let ordinary = note("selected", 2, false, true);
    let pitch = "<pitch><step>G</step><octave>4</octave></pitch>";
    let cases = [
        ordinary.replace("<pitch>", "<cue/><pitch>"),
        ordinary.replace(pitch, "<unpitched><display-step>G</display-step><display-octave>4</display-octave></unpitched>"),
    ];
    for body in cases {
        // These mappings are not supported yet. Preserve that explicit error;
        // never remove authored data to manufacture a playable fixture.
        assert!(MusicXml.read(&xml(&[body], 1)).is_err());
    }
    let body = ordinary.replace(
        "<notations>",
        "<notations><articulations><staccato/></articulations>",
    );
    let s = source(&xml(&[body], 1));
    let error = play(&s, &policy(&s, &[("1", "1")])).unwrap_err();
    assert!(
        error.contains("fermata.articulation_unsupported"),
        "{error}"
    );
}
#[test]
fn final_hold_exact_inverse_seek_and_clamping_share_the_audio_clock() {
    let s = intervals(&[(0, 2, false)], 1);
    let p = play(&s, &policy(&s, &[("1", "1/3")])).unwrap();
    assert_eq!(
        p.timing.seek(&r("2/3")),
        SeekPosition::Hold {
            group_id: "explicit-0".into(),
            boundary: r("1"),
            elapsed_seconds: r("1/6")
        }
    );
    assert_eq!(
        p.timing.seek(&r("5/6")),
        SeekPosition::Music { quarter: r("1") }
    );
    assert_eq!(
        p.timing.seek(&r("100")),
        SeekPosition::Music { quarter: r("1") }
    );
    assert_eq!(
        p.timing.seek(&r("-1")),
        SeekPosition::Music { quarter: r("0") }
    );
    close(p.plan.duration_seconds, 5. / 6.);
    close(p.plan.notes[0].duration_seconds, 5. / 6.);
}
#[test]
fn numeric_boundary_step_stays_old_through_hold_then_changes_at_release() {
    let s = tied(0, true);
    let p = play(&s, &policy(&s, &[("1", "1")])).unwrap();
    let n = &p.plan.notes[0];
    assert_eq!(n.velocity, 45);
    assert!(
        n.gain_envelope
            .windows(2)
            .any(|pair| (pair[0].offset_seconds - 1.5).abs() < 1e-9
                && pair[0].offset_seconds == pair[1].offset_seconds
                && pair[0].gain == 1.
                && pair[1].gain == 2.)
    );
    for e in n.gain_envelope.iter().filter(|e| e.offset_seconds < 1.5) {
        close(e.gain, 1.);
    }
}
#[test]
fn repeated_occurrence_policies_keep_checkpoint_tempo_dynamics_and_pitch() {
    let xml = include_str!("fixtures/navigation-performed.musicxml").replace(
        "<type>quarter</type></note>",
        "<type>quarter</type><notations><fermata/></notations></note>",
    );
    let s = source(&xml);
    let d = discover_source_fermatas(&s).unwrap();
    assert_eq!(d.authored.len(), 5);
    assert_eq!(d.occurrences.len(), 6);
    let p = play(
        &s,
        &policy(
            &s,
            &[
                ("1", "1"),
                ("2", "1"),
                ("3", "1"),
                ("4", "2"),
                ("5", "1"),
                ("6", "1"),
            ],
        ),
    )
    .unwrap();
    assert_eq!(
        p.plan
            .notes
            .iter()
            .map(|n| n.written_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["before", "a", "b", "a", "c", "d"]
    );
    assert_eq!(
        p.plan.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [45, 45, 90, 45, 45, 45]
    );
    assert_eq!(
        p.plan.notes.iter().map(|n| n.midi_note).collect::<Vec<_>>(),
        [57., 58., 59., 58., 62., 63.]
    );
    assert_ne!(p.plan.notes[1].id, p.plan.notes[3].id);
    for (n, t) in p.plan.notes.iter().zip([0., 2., 4., 5.5, 8.5, 10.5]) {
        close(n.start_seconds, t);
    }
    close(p.plan.notes[1].duration_seconds, 2.);
    close(p.plan.notes[3].duration_seconds, 3.);
    close(p.plan.duration_seconds, 12.5);
}
#[test]
fn desktop_fixture_is_the_actual_qualified_source_plan() {
    let s = source(include_str!("fixtures/fermata-playback.musicxml"));
    let p = play(&s, &policy(&s, &[("1", "1/4"), ("2", "1/2"), ("4", "3/4")])).unwrap();
    assert_eq!(p.plan.notes.len(), 1);
    close(p.plan.notes[0].duration_seconds, 7. / 4.);
    close(p.plan.duration_seconds, 7. / 2.);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/tests/fixtures/fermata-playback.json");
    let actual = serde_json::to_value(&p.plan).unwrap();
    if std::env::var_os("UPDATE_FERMATA_FIXTURE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let checked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        checked, actual,
        "Regenerate only from the unchanged MusicXML fixture with UPDATE_FERMATA_FIXTURE=1"
    );
}

#[test]
fn selection_at_one_hold_does_not_exempt_chain_at_another_hold() {
    let other = note("other-lead", 2, true, false)
        + &note("other-anchor", 2, false, true)
        + &note("other-tail", 2, true, false);
    let s = source(&xml(&[tied_body(0, false), other], 3));
    let error = play(&s, &policy(&s, &[("1", "1"), ("2", "1")])).unwrap_err();
    assert!(error.contains("fermata.unselected_overlap"), "{error}");
}
