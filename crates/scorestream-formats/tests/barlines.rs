use scorestream_core::{
    Rational,
    notation::PartId,
    repeat_traversal::{self as route, Policy},
    score::*,
};
use scorestream_formats::{
    ScoreAdapter, musicxml::MusicXml, navigation, sfs::SfsAdapter, validate_score,
};
fn bar(inner: &str, side: &str) -> String {
    format!("<barline location=\"{side}\">{inner}</barline>")
}
fn note() -> &'static str {
    "<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note>"
}
fn measure(i: usize, left: &str, right: &str) -> String {
    format!(
        "<measure number=\"{}\">{left}<attributes><divisions>4</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes>{}{right}</measure>",
        i + 1,
        note()
    )
}
fn document(parts: &[String]) -> String {
    let mut s = String::from("<score-partwise version=\"4.0\"><part-list>");
    for i in 0..parts.len() {
        s.push_str(&format!("<score-part id=\"P{}\"><part-name>Piano</part-name><score-instrument id=\"I{}\"><instrument-name>piano</instrument-name></score-instrument></score-part>",i+1,i+1));
    }
    s.push_str("</part-list>");
    for (i, p) in parts.iter().enumerate() {
        s.push_str(&format!("<part id=\"P{}\">{p}</part>", i + 1));
    }
    s.push_str("</score-partwise>");
    s
}
fn ordinary() -> String {
    document(&[(0..4).map(|i|measure(i,if i==1 {"<barline location=\"left\"><bar-style>heavy-light</bar-style><repeat direction=\"forward\"/></barline>"}else{""},if i==2 {"<barline><bar-style>light-heavy</bar-style><repeat direction=\"backward\" times=\"3\"/></barline>"}else{""})).collect()])
}
fn volta(close: bool, parts: usize) -> String {
    document(&(0..parts).map(|p|(0..5).map(|i| {
        let left=if i==1 {bar("<bar-style>heavy-light</bar-style><repeat direction=\"forward\"/>","left")}else if p==0&&i==2 {bar("<ending number=\"1\" type=\"start\" default-y=\"77.7\">First &amp; only</ending>","left")}else if p==0&&i==3 {bar("<ending number=\"2\" type=\"start\"/>","left")}else{String::new()};
        let right=if i==2 {bar(&format!("<bar-style>light-heavy</bar-style>{}<repeat direction=\"backward\"/>",if p==0 {"<ending number=\"1\" type=\"stop\"/>"}else{""}),"right")}else if p==0&&i==3&&close {bar("<ending number=\"2\" type=\"discontinue\"/>","right")}else{String::new()};
        measure(i,&left,&right)
    }).collect()).collect::<Vec<_>>())
}
fn visits(score: &ScoreDocument, policy: Policy) -> Vec<usize> {
    navigation::plan(score, "movement", policy)
        .unwrap()
        .occurrences
        .iter()
        .map(|o| o.measure_index)
        .collect()
}
fn has(report: &scorestream_formats::MappingReport, feature: &str) {
    assert!(
        report.diagnostics.iter().any(|d| d.feature == feature),
        "{report:?}"
    );
}
#[test]
fn ordinary_xml_model_roundtrip_and_exact_replay() {
    let s = MusicXml.read(&ordinary()).unwrap().value;
    assert_eq!(s.barlines.len(), 2);
    let plan = navigation::plan(&s, "ordinary", Policy::default()).unwrap();
    assert_eq!(visits(&s, Policy::default()), [0, 1, 2, 1, 2, 1, 2, 3]);
    assert_eq!(plan.duration_quarters, "8".parse().unwrap());
    assert!(plan.traversal.diagnostics.is_empty());
    assert_eq!(plan.occurrences[3].written_start, Rational::one());
    assert_eq!(plan.occurrences[3].performed_start, "3".parse().unwrap());
    assert_eq!(
        plan.traversal
            .steps
            .iter()
            .filter(|s| matches!(s, route::Step::RestoreCheckpoint { .. }))
            .count(),
        2
    );
    let again = MusicXml
        .read(&MusicXml.write(&s).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(s.barlines, again.barlines);
    assert_eq!(s.parts[0].measures, again.parts[0].measures);
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&s).unwrap().value)
            .unwrap()
            .event_positions
            .len(),
        8
    );
}
#[test]
fn agrees_across_parts_and_preserves_ending_provenance() {
    let s = MusicXml.read(&volta(true, 3)).unwrap().value;
    let plan = navigation::plan(&s, "volta", Policy::default()).unwrap();
    assert_eq!(plan.navigation.repeat_parts.len(), 3);
    assert_eq!(plan.navigation.ending_parts, [PartId("P1".into())]);
    assert_eq!(visits(&s, Policy::default()), [0, 1, 2, 1, 3, 4]);
    assert_eq!(plan.duration_quarters, "6".parse().unwrap());
    let ending = s
        .barlines
        .iter()
        .find_map(|b| b.ending.as_ref().filter(|e| e.phase == EndingPhase::Start))
        .unwrap();
    assert_eq!(ending.text, "First & only");
    assert_eq!(ending.default_y, Some("777/10".parse().unwrap()));
    let again = MusicXml
        .read(&MusicXml.write(&s).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(s.barlines, again.barlines);
    assert_eq!(s, again);
}
#[test]
fn unclosed_final_ending_survives_and_default_policy_rejects() {
    let s = MusicXml.read(&volta(false, 2)).unwrap().value;
    let graph = navigation::from_score(&s, "open").unwrap();
    assert_eq!(
        graph.movement.repeats[0].endings[1].end,
        route::EndingEnd::Unclosed
    );
    let e = navigation::plan(&s, "open", Policy::default()).unwrap_err();
    assert!(e.diagnostics[0].message.contains("UnclosedFinalEnding"));
    let p = Policy {
        unclosed_final_ending: route::UnclosedFinalEndingPolicy::ContinueToMovementEnd,
        ..Policy::default()
    };
    let plan = navigation::plan(&s, "open", p).unwrap();
    assert_eq!(visits(&s, p), [0, 1, 2, 1, 3, 4]);
    assert!(
        plan.traversal
            .diagnostics
            .iter()
            .any(|d| matches!(d, route::Diagnostic::UnclosedFinalEndingContinues { .. }))
    );
    assert_eq!(
        s.barlines,
        MusicXml
            .read(&MusicXml.write(&s).unwrap().value)
            .unwrap()
            .value
            .barlines
    );
}
#[test]
fn implicit_start_count_policy_and_non_numeric_labels() {
    let src = ordinary()
        .replace("<repeat direction=\"forward\"/>", "")
        .replace(" times=\"3\"", "")
        .replace("number=\"1\"", "number=\"0\"")
        .replace("number=\"2\"", "number=\"1a\"");
    let s = MusicXml.read(&src).unwrap().value;
    let p = navigation::plan(&s, "implicit", Policy::default()).unwrap();
    assert_eq!(visits(&s, Policy::default()), [0, 1, 2, 0, 1, 2, 3]);
    assert_eq!(p.traversal.diagnostics.len(), 2);
    assert_eq!(p.navigation.movement.measure_labels[1], "1a");
    assert!(
        navigation::plan(
            &s,
            "implicit",
            Policy {
                allow_implicit_movement_start: false,
                ..Policy::default()
            }
        )
        .is_err()
    );
}
#[test]
fn strict_unknown_and_malformed_barlines_do_not_pass() {
    for (src, feature) in [
        (
            ordinary().replace("location=\"left\"", "location=\"middle\""),
            "barline.middle",
        ),
        (
            ordinary().replace(
                "<repeat direction=\"forward\"/>",
                "<repeat direction=\"forward\" winged=\"straight\"/>",
            ),
            "attribute.winged",
        ),
        (
            ordinary().replace(
                "<repeat direction=\"forward\"/>",
                "<repeat direction=\"forward\" times=\"2\"/>",
            ),
            "repeat.times_direction",
        ),
        (
            ordinary().replace(
                "<repeat direction=\"forward\"/>",
                "<repeat direction=\"sideways\"/>",
            ),
            "repeat.direction",
        ),
        (
            ordinary().replace("times=\"3\"", "times=\"-1\""),
            "repeat.times",
        ),
        (
            volta(true, 1).replace(
                "number=\"1\" type=\"start\"",
                "number=\"1,1\" type=\"start\"",
            ),
            "ending.passes",
        ),
        (
            volta(true, 1).replace("number=\"1\" type=\"start\"", "number=\" \" type=\"start\""),
            "ending.unknown_passes",
        ),
        (
            ordinary().replace(
                "<bar-style>heavy-light</bar-style>",
                "<bar-style>unknown</bar-style>",
            ),
            "barline.style",
        ),
        (
            ordinary().replace("<bar-style>heavy-light</bar-style>", "<fermata/>"),
            "element.fermata",
        ),
        (
            volta(true, 1).replace("default-y=\"77.7\"", "default-y=\"NaN\""),
            "number.decimal",
        ),
    ] {
        has(&MusicXml.read(&src).unwrap_err(), feature);
    }
    let s = ordinary().replace(
        "<barline location=\"left\">",
        "<barline location=\"right\">",
    );
    has(&MusicXml.read(&s).unwrap_err(), "barline.position");
}
#[test]
fn contradictory_part_navigation_and_public_corruption_are_rejected() {
    let mut s = MusicXml.read(&volta(true, 2)).unwrap().value;
    s.barlines
        .iter_mut()
        .find(|b| {
            b.part.0 == "P2"
                && b.repeat
                    .as_ref()
                    .is_some_and(|r| r.direction == RepeatDirection::Backward)
        })
        .unwrap()
        .repeat
        .as_mut()
        .unwrap()
        .times = Some(3);
    has(
        &navigation::plan(&s, "conflict", Policy::default()).unwrap_err(),
        "navigation.repeat_conflict",
    );
    let mut s = MusicXml.read(&ordinary()).unwrap().value;
    s.barlines[0].part = PartId("missing".into());
    has(&validate_score(&s), "barline.measure_reference");
    let mut s = MusicXml.read(&ordinary()).unwrap().value;
    s.barlines.push(s.barlines[0].clone());
    has(&validate_score(&s), "barline.duplicate_boundary");
    let mut s = MusicXml.read(&ordinary()).unwrap().value;
    s.barlines[0].id = Some(s.parts[0].measures[0].id.clone());
    has(&validate_score(&s), "barline.identity");
    let mut s = MusicXml.read(&volta(true, 1)).unwrap().value;
    s.barlines
        .iter_mut()
        .find_map(|b| b.ending.as_mut())
        .unwrap()
        .passes = vec![0];
    has(&validate_score(&s), "ending.passes");
}
#[test]
fn public_exact_placement_export_and_identity_preflight() {
    let mut s = MusicXml.read(&volta(true, 1)).unwrap().value;
    s.barlines[0].id = Some("repeat_head".into());
    assert_eq!(
        s.barlines,
        MusicXml
            .read(&MusicXml.write(&s).unwrap().value)
            .unwrap()
            .value
            .barlines
    );
    let e = s
        .barlines
        .iter_mut()
        .find_map(|b| b.ending.as_mut())
        .unwrap();
    e.default_y = Some("1/3".parse().unwrap());
    has(&MusicXml.write(&s).unwrap_err(), "ending.default_y");
    s.barlines
        .iter_mut()
        .find_map(|b| b.ending.as_mut())
        .unwrap()
        .default_y = Some(format!("1{}", "0".repeat(64)).parse().unwrap());
    has(&MusicXml.write(&s).unwrap_err(), "ending.default_y");
}
#[test]
fn malformed_relationships_and_budget_return_no_plan() {
    for src in [
        volta(true, 1).replace("number=\"1\" type=\"stop\"", "number=\"2\" type=\"stop\""),
        ordinary().replace("<repeat direction=\"backward\" times=\"3\"/>", ""),
        ordinary().replace("times=\"3\"", "times=\"0\""),
        volta(true, 1).replace(
            "<repeat direction=\"backward\"/>",
            "<repeat direction=\"backward\" times=\"2\"/>",
        ),
    ] {
        let s = MusicXml.read(&src).unwrap().value;
        assert!(navigation::plan(&s, "bad", Policy::default()).is_err());
    }
    // This is valid authored data. Exercise the caller budget without
    // materializing billions of visits under the unbounded default.
    let source = ordinary().replace("times=\"3\"", "times=\"4294967295\"");
    let extreme = MusicXml.read(&source).unwrap().value;
    let error = navigation::plan(
        &extreme,
        "explicit-budget",
        Policy {
            maximum_occurrences: 1_000,
            ..Policy::default()
        },
    )
    .unwrap_err();
    assert!(error.diagnostics.iter().any(|d| {
        d.feature == "navigation.traversal" && d.message.contains("OccurrenceBudget")
    }));
    let s = MusicXml.read(&ordinary()).unwrap().value;
    assert!(
        navigation::plan(
            &s,
            "budget",
            Policy {
                maximum_occurrences: 7,
                ..Policy::default()
            }
        )
        .is_err()
    );
    assert!(
        navigation::plan(
            &s,
            "budget",
            Policy {
                maximum_occurrences: 8,
                ..Policy::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn real_tempo_controls_follow_checkpoint_and_skip_contract_without_source_rewrite() {
    let source=volta(true,1).replace("<measure number=\"1\">","<measure number=\"1\"><sound tempo=\"60\"/>").replace("<ending number=\"1\" type=\"start\" default-y=\"77.7\">First &amp; only</ending></barline>","<ending number=\"1\" type=\"start\" default-y=\"77.7\">First &amp; only</ending></barline><sound tempo=\"200\"/>");
    let score = MusicXml.read(&source).unwrap().value;
    assert_eq!(score.tempos.len(), 2);
    let source_copy = score.clone();
    let plan = navigation::plan(&score, "state", Policy::default()).unwrap();
    let mut checkpoints = std::collections::BTreeMap::new();
    let mut tempo = Rational::from_integer(120);
    let mut observed = vec![];
    for step in &plan.traversal.steps {
        match step {
            route::Step::CaptureCheckpoint { repeat_index, .. } => {
                checkpoints.insert(*repeat_index, tempo.clone());
            }
            route::Step::RestoreCheckpoint { repeat_index, .. } => {
                tempo = checkpoints[repeat_index].clone()
            }
            route::Step::ReplayMeasure { measure_index, .. } => {
                let m = &score.parts[0].measures[*measure_index];
                for change in score
                    .tempos
                    .iter()
                    .filter(|t| t.at >= m.start && t.at < &m.start + &m.length)
                {
                    tempo = change.quarter_bpm.clone();
                }
                observed.push(tempo.to_string());
            }
            _ => {}
        }
    }
    assert_eq!(observed, ["60", "60", "200", "60", "60", "60"]);
    assert_eq!(
        score, source_copy,
        "Planning must not duplicate or mutate authored controls"
    );
    let again = MusicXml
        .read(&MusicXml.write(&score).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(score.barlines, again.barlines);
    assert_eq!(score.tempos, again.tempos);
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .measures
            .len(),
        6
    );
}
#[test]
fn adjacency_multi_pass_endings_and_source_part_order_are_exact() {
    let source = volta(true, 2)
        .replace("number=\"1\" type=", "number=\"1,2\" type=")
        .replace("number=\"2\" type=", "number=\"3\" type=");
    let score = MusicXml.read(&source).unwrap().value;
    assert_eq!(visits(&score, Policy::default()), [0, 1, 2, 1, 2, 1, 3, 4]);
    let a = source.find("<part id=\"P1\">").unwrap();
    let b = source.find("<part id=\"P2\">").unwrap();
    let end = source.find("</score-partwise>").unwrap();
    let reversed = format!(
        "{}{}{}{}</score-partwise>",
        &source[..a],
        &source[b..end],
        &source[a..b],
        ""
    );
    let reversed = MusicXml.read(&reversed).unwrap().value;
    assert_eq!(score.barlines, reversed.barlines);
    let part = (0..4)
        .map(|i| {
            measure(
                i,
                &if i == 0 || i == 2 {
                    bar("<repeat direction=\"forward\"/>", "left")
                } else {
                    String::new()
                },
                &if i == 1 || i == 3 {
                    bar("<repeat direction=\"backward\" times=\"2\"/>", "right")
                } else {
                    String::new()
                },
            )
        })
        .collect();
    let score = MusicXml.read(&document(&[part])).unwrap().value;
    assert_eq!(visits(&score, Policy::default()), [0, 1, 0, 1, 2, 3, 2, 3]);
}
