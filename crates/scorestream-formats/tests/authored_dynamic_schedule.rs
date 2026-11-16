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
    let mut next = attack("ending-visit", "c", "m3", 3, "3", "3");
    next.part = "P1".into();
    assert_eq!(
        resolve(&xml, &[next]).decisions[0].disposition,
        scorestream_core::notation::PracticeAttackDisposition::NoProvenOpeningVisit
    );
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

fn attack(
    id: &str,
    event: &str,
    measure: &str,
    visit: usize,
    onset: &str,
    chain: &str,
) -> scorestream_core::notation::QualifiedPracticeAttack {
    scorestream_core::notation::QualifiedPracticeAttack {
        occurrence_id: id.into(),
        written_event_id: event.into(),
        part: "p".into(),
        staff: 1,
        written_measure: measure.into(),
        measure_occurrence: visit,
        onset: onset.parse().unwrap(),
        chain_attack: chain.parse().unwrap(),
        existing_attack_percent: None,
    }
}
fn resolve(
    xml: &str,
    attacks: &[scorestream_core::notation::QualifiedPracticeAttack],
) -> scorestream_core::notation::PracticeAttackResolution {
    let m = MusicXml.read(xml).unwrap().value;
    let doc = to_document(&m).unwrap().value;
    let before = doc.clone();
    let result=scorestream_core::notation::resolve_practice_boundary_attacks(&doc,attacks,scorestream_core::notation::PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,PerformanceLimits::default()).unwrap();
    assert_eq!(doc, before);
    assert!(result.schedule.execution_withheld);
    assert!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&m).unwrap().value)
            .unwrap_err()
            .contains("authored_dynamics.performance_required")
    );
    result
}
fn split_opening(extra: &str) -> String {
    pair(false,false,false).replace(r#"<note id="b"><pitch><step>D</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice></note>"#,&format!(r#"<note id="b"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>{extra}<note id="b2"><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>"#))
}
#[test]
fn practice_percent_applies_only_to_new_attacks_within_the_proven_opening_visit() {
    use scorestream_core::notation::PracticeAttackDisposition as D;
    let attacks = [
        attack("before", "a", "m1", 0, "0", "0"),
        attack("opening", "b", "m2", 1, "1", "1"),
        attack("later", "b2", "m2", 1, "4/3", "4/3"),
        attack("after", "outside", "m2", 1, "2", "2"),
    ];
    let result = resolve(&split_opening(""), &attacks);
    assert_eq!(
        result
            .decisions
            .iter()
            .map(|d| d.disposition)
            .collect::<Vec<_>>(),
        [
            D::NoProvenOpeningVisit,
            D::Selected,
            D::Selected,
            D::NoProvenOpeningVisit
        ]
    );
    for d in &result.decisions[1..3] {
        assert_eq!(d.percent.as_ref().unwrap().to_string(), "10667/100");
        let [a, b] = d.terminal_then_opening.unwrap();
        assert_eq!(
            result.schedule.statements[a].source.percent.to_string(),
            "1111/25"
        );
        assert_eq!(
            result.schedule.statements[b].source.percent.to_string(),
            "10667/100"
        );
    }
}
#[test]
fn practice_resolver_does_not_retrigger_ties_or_update_existing_sounding_notes() {
    use scorestream_core::notation::PracticeAttackDisposition as D;
    let xml=pair(false,false,false).replace("<step>D</step>","<step>C</step>")
 .replacen("<duration>3</duration><voice>1</voice></note>","<duration>3</duration><tie type=\"start\"/><voice>1</voice><notations><tied type=\"start\"/></notations></note>",1);
    let close = xml
        .rfind("<duration>3</duration><voice>1</voice></note>")
        .unwrap();
    let mut xml = xml;
    xml.replace_range(close..close+"<duration>3</duration><voice>1</voice></note>".len(),"<duration>3</duration><tie type=\"stop\"/><voice>1</voice><notations><tied type=\"stop\"/></notations></note>");
    let result = resolve(
        &xml,
        &[
            attack("held", "a", "m1", 0, "0", "0"),
            attack("tie-continuation", "b", "m2", 1, "1", "0"),
        ],
    );
    assert_eq!(result.decisions[0].disposition, D::NoProvenOpeningVisit);
    assert_eq!(
        result.decisions[1].disposition,
        D::ContinuationOrAlreadySounding
    );
    let incorrectly_new=resolve(&xml,&[attack("tie-source-protected","b","m2",1,"1","1")]);
    assert_eq!(incorrectly_new.decisions[0].disposition,D::ContinuationOrAlreadySounding);
    assert!(result.decisions.iter().all(|d| d.percent.is_none()));
}
#[test]
fn practice_resolver_stops_at_competing_declaration_and_protects_existing_note_percent() {
    use scorestream_core::notation::PracticeAttackDisposition as D;
    let extra = r#"<direction><direction-type><dynamics><f/></dynamics></direction-type><sound dynamics="87.78"/></direction>"#;
    let result = resolve(
        &split_opening(extra),
        &[
            attack("opening", "b", "m2", 1, "1", "1"),
            attack("later", "b2", "m2", 1, "4/3", "4/3"),
        ],
    );
    assert_eq!(result.decisions[0].disposition, D::Selected);
    assert_eq!(result.decisions[1].disposition, D::CompetingStatement);
    assert!(result.decisions[1].percent.is_none());
    let mut a = attack("explicit", "b", "m2", 1, "1", "1");
    a.existing_attack_percent = Some("55".parse().unwrap());
    assert_eq!(
        resolve(&pair(false, false, false), &[a]).decisions[0].disposition,
        D::ExistingAttackPercentage
    );
}
#[test]
fn practice_resolver_never_carries_over_a_repeat_jump_but_uses_each_proven_exit() {
    use scorestream_core::notation::PracticeAttackDisposition as D;
    let result = resolve(
        &pair(false, true, true),
        &[
            attack("jump-opening", "a", "m1", 1, "1", "1"),
            attack("actual-exit", "b", "m2", 2, "2", "2"),
        ],
    );
    assert_eq!(result.decisions[0].disposition, D::UnresolvedBoundary);
    assert_eq!(result.decisions[1].disposition, D::Selected);
    let result = resolve(
        &pair(true, false, false),
        &[
            attack("first", "b", "m2", 1, "1", "1"),
            attack("second", "b", "m2", 3, "3", "3"),
        ],
    );
    assert!(result
        .decisions
        .iter()
        .all(|d| d.disposition == D::Selected));
    assert_ne!(
        result.decisions[0].terminal_then_opening,
        result.decisions[1].terminal_then_opening
    );
}
#[test]
fn practice_resolver_preserves_competing_legacy_numeric_control() {
    use scorestream_core::{Record, Scalar, SourceRange, State};
    let m = MusicXml.read(&pair(false, false, false)).unwrap().value;
    let mut doc = to_document(&m).unwrap().value;
    let frame = doc
        .frames
        .iter_mut()
        .find(|f| f.time.to_string() == "1")
        .unwrap();
    frame.records.push(Record::State(State {
        target: "p".into(),
        attributes: std::collections::BTreeMap::from([(
            "sound_dynamics".into(),
            Scalar::String("55".into()),
        )]),
        source: SourceRange::default(),
    }));
    let before = doc.clone();
    let result=scorestream_core::notation::resolve_practice_boundary_attacks(&doc,&[attack("existing-control","b","m2",1,"1","1")],scorestream_core::notation::PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,PerformanceLimits::default()).unwrap();
    assert_eq!(
        result.decisions[0].disposition,
        scorestream_core::notation::PracticeAttackDisposition::LegacyControlConflict
    );
    assert!(result.decisions[0].percent.is_none());
    assert_eq!(doc, before);
}
#[test]
fn practice_resolver_requires_exact_source_attack_identity_time_and_scope() {
    use scorestream_core::notation::PracticeAttackDisposition as D;
    let result = resolve(
        &pair(false, false, false),
        &[
            attack("wrong-time", "b", "m2", 1, "4/3", "4/3"),
            attack("unknown-id", "missing", "m2", 1, "1", "1"),
        ],
    );
    assert!(result
        .decisions
        .iter()
        .all(|d| d.disposition == D::UnverifiedAttackFacts && d.percent.is_none()));
}
