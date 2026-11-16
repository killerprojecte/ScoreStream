use scorestream_core::notation::{
    inspect_authored_dynamic_schedule, AuthoredDynamicSchedulePolicy, BoundaryOrdering,
    PerformanceLimits,
};
use scorestream_formats::{
    musicxml::MusicXml,
    sfs::{to_document, SfsAdapter},
    ScoreAdapter,
};
fn pair(repeat_pair: bool, repeat_first: bool, initial: bool) -> String {
    let head = if repeat_pair || repeat_first {
        r#"<barline location="left"><repeat direction="forward"/></barline>"#
    } else {
        ""
    };
    let tail = r#"<barline location="right"><repeat direction="backward" times="2"/></barline>"#;
    let begin = if initial {
        r#"<direction><direction-type><dynamics><mp/></dynamics></direction-type><sound dynamics="60"/></direction>"#
    } else {
        ""
    };
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m1" number="1">{head}<attributes><divisions>3</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes>{begin}<note id="a"><pitch><step>C</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice></note><direction><direction-type><dynamics><p/></dynamics></direction-type><offset>-1</offset><sound dynamics="44.44"/></direction>{}</measure><measure id="m2" number="2"><direction><direction-type><dynamics><ff/></dynamics></direction-type><sound dynamics="106.67"/></direction><note id="b"><pitch><step>D</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice></note>{}</measure></part></score-partwise>"#,
        if repeat_first { tail } else { "" },
        if repeat_pair { tail } else { "" }
    )
}
fn inspect(xml: &str) -> scorestream_core::notation::AuthoredDynamicSchedule {
    let model = MusicXml.read(xml).unwrap().value;
    let doc = to_document(&model).unwrap().value;
    let before = serde_json::to_value(&doc).unwrap();
    let result = inspect_authored_dynamic_schedule(
        &doc,
        AuthoredDynamicSchedulePolicy::AdjacentWrittenOwnersV1,
        PerformanceLimits::default(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&doc).unwrap(), before);
    assert!(result.execution_withheld);
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&sfs)
        .unwrap_err()
        .contains("authored_dynamics.performance_required"));
    result
}
#[test]
fn ordinary_pair_retains_both_values_without_constructing_gains() {
    let s = inspect(&pair(false, false, false));
    assert_eq!(s.statements.len(), 2);
    assert_eq!(s.boundaries.len(), 1);
    let b = &s.boundaries[0];
    assert_eq!(b.ordering, BoundaryOrdering::AdjacentTerminalBeforeOpening);
    let [a, c] = b.terminal_then_opening.unwrap();
    assert_eq!(s.statements[a].source.percent.to_string(), "1111/25");
    assert_eq!(s.statements[c].source.percent.to_string(), "10667/100");
    assert_eq!(s.statements[a].source.visual_at.to_string(), "2/3");
    assert_eq!(s.statements[a].performed_sounding_at.to_string(), "1");
    let json = serde_json::to_value(&s).unwrap();
    assert!(json.get("notes").is_none());
    assert!(json.get("gains").is_none());
    assert_eq!(
        serde_json::from_value::<scorestream_core::notation::AuthoredDynamicSchedule>(json)
            .unwrap(),
        s
    );
}
#[test]
fn complete_repeat_visits_get_independent_owner_pairs() {
    let s = inspect(&pair(true, false, false));
    assert_eq!(s.statements.len(), 4);
    assert_eq!(s.boundaries.len(), 2);
    assert_eq!(
        s.boundaries
            .iter()
            .map(|b| b.at.to_string())
            .collect::<Vec<_>>(),
        ["1", "3"]
    );
    for b in &s.boundaries {
        assert_eq!(b.ordering, BoundaryOrdering::AdjacentTerminalBeforeOpening);
        let [a, c] = b.terminal_then_opening.unwrap();
        assert_eq!(s.statements[a].part_visit + 1, s.statements[c].part_visit);
        assert_eq!(s.statements[a].source_index, 0);
        assert_eq!(s.statements[c].source_index, 1);
    }
}
#[test]
fn backward_jump_to_same_written_owner_does_not_gain_adjacent_precedence() {
    let s = inspect(&pair(false, true, true));
    let jump = s
        .boundaries
        .iter()
        .find(|b| b.at.to_string() == "1")
        .unwrap();
    assert_eq!(jump.ordering, BoundaryOrdering::Unresolved);
    assert_eq!(jump.terminal_then_opening, None);
    let next = s
        .boundaries
        .iter()
        .find(|b| b.at.to_string() == "2")
        .unwrap();
    assert_eq!(
        next.ordering,
        BoundaryOrdering::AdjacentTerminalBeforeOpening
    );
    assert!(s
        .diagnostics
        .iter()
        .any(|d| d.code == "authored_dynamics.unresolved_visit"));
}
#[test]
fn skipped_first_ending_does_not_pair_separate_visits_by_written_clock() {
    let xml=include_str!("fixtures/navigation-closed.musicxml")
  .replace("<barline location=\"right\"><bar-style>light-heavy</bar-style><ending number=\"1\"",r#"<direction><direction-type><dynamics><p/></dynamics></direction-type><offset>-0.25</offset><sound dynamics="44.44"/></direction><barline location="right"><bar-style>light-heavy</bar-style><ending number="1""#)
  .replace("<note id=\"c\">",r#"<direction><direction-type><dynamics><ff/></dynamics></direction-type><sound dynamics="106.67"/></direction><note id="c">"#);
    let s = inspect(&xml);
    assert_eq!(s.statements.len(), 2);
    assert_eq!(
        s.statements
            .iter()
            .map(|e| e.performed_sounding_at.to_string())
            .collect::<Vec<_>>(),
        ["2", "3"]
    );
    assert!(s.boundaries.iter().all(
        |b| b.ordering == BoundaryOrdering::IsolatedExact && b.terminal_then_opening.is_none()
    ));
    assert_eq!(
        s.diagnostics
            .iter()
            .filter(|d| d.code == "authored_dynamics.unpaired_owner_visit")
            .count(),
        2
    );
}
#[test]
fn occurrence_budgets_are_explicit_and_source_is_unchanged() {
    let m = MusicXml.read(&pair(true, false, false)).unwrap().value;
    let doc = to_document(&m).unwrap().value;
    let before = doc.clone();
    let limits = PerformanceLimits {
        maximum_controls: 1,
        ..PerformanceLimits::default()
    };
    assert!(inspect_authored_dynamic_schedule(
        &doc,
        AuthoredDynamicSchedulePolicy::AdjacentWrittenOwnersV1,
        limits
    )
    .is_err());
    assert_eq!(doc, before);
}
#[test]
fn three_simultaneous_statements_remain_unresolved_without_last_wins() {
    let extra = r#"<direction><direction-type><dynamics><f/></dynamics></direction-type><offset>-1</offset><sound dynamics="87.78"/></direction>"#;
    let xml = pair(false, false, false).replace(
        "</direction></measure><measure id=\"m2\"",
        &format!("</direction>{extra}</measure><measure id=\"m2\""),
    );
    let s = inspect(&xml);
    assert_eq!(s.written_statements.len(), 3);
    assert_eq!(s.statements.len(), 3);
    assert_eq!(s.boundaries[0].ordering, BoundaryOrdering::Unresolved);
    assert_eq!(s.boundaries[0].terminal_then_opening, None);
    assert_eq!(s.boundaries[0].statement_indices.len(), 3);
}
#[test]
fn final_terminal_is_retained_as_exact_isolated_statement() {
    let xml = pair(false, false, false);
    let one =
        xml.split("<measure id=\"m2\"").next().unwrap().to_owned() + "</part></score-partwise>";
    let s = inspect(&one);
    assert_eq!(s.statements.len(), 1);
    assert_eq!(s.boundaries[0].ordering, BoundaryOrdering::IsolatedExact);
    assert_eq!(s.boundaries[0].at.to_string(), "1");
    assert_eq!(s.written_statements[0].percent.to_string(), "1111/25");
    assert!(s
        .diagnostics
        .iter()
        .any(|d| d.code == "authored_dynamics.unpaired_owner_visit"));
}
