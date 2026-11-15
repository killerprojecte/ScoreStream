use scorestream_core::{
    Rational,
    notation::{EventId, Placement, RelationId, VoiceId},
    score::{ScoreDocument, ScoreVoice, SlurOrientation},
};
use scorestream_formats::{MappingEffect, ScoreAdapter, musicxml::MusicXml};

const PITCH: &str = "<pitch><step>C</step><octave>4</octave></pitch>";
fn r(value: &str) -> Rational {
    value.parse().unwrap()
}
fn xml(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><staves>2</staves></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, voice: &str, staff: u32, slurs: &str) -> String {
    format!(
        r#"<note id="{id}">{PITCH}<duration>1</duration><voice>{voice}</voice><staff>{staff}</staff>{}</note>"#,
        if slurs.is_empty() {
            String::new()
        } else {
            format!("<notations>{slurs}</notations>")
        }
    )
}
fn grace(id: &str, slurs: &str) -> String {
    note(id, "1", 1, slurs).replace("<duration>1</duration>", "<grace slash=\"yes\"/>")
}
fn mark(phase: &str, number: u8) -> String {
    format!(r#"<slur type="{phase}" number="{number}"/>"#)
}
fn pair(start: &str, end: &str) -> String {
    xml(&format!(
        "{}{}",
        note("a", "1", 1, start),
        note("b", "1", 1, end)
    ))
}
fn read(source: &str) -> ScoreDocument {
    MusicXml.read(source).unwrap().value
}
fn signatures(score: &ScoreDocument) -> Vec<String> {
    let mut signatures: Vec<_> = score
        .slurs
        .iter()
        .map(|slur| serde_json::to_string(&(&slur.start, &slur.end)).unwrap())
        .collect();
    signatures.sort();
    signatures
}
fn roundtrip(score: &ScoreDocument) -> String {
    let exported = MusicXml.write(score).unwrap();
    assert!(
        exported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.identity" && d.effect == MappingEffect::Normalized)
    );
    let again = read(&exported.value);
    assert_eq!(signatures(score), signatures(&again));
    assert_eq!(score.graces, again.graces);
    exported.value
}
fn error(source: &str, feature: &str) {
    let report = MusicXml.read(source).unwrap_err();
    assert!(
        report.diagnostics.iter().any(|d| d.feature == feature),
        "Expected {feature}: {report:?}"
    );
}

#[test]
fn exact_endpoint_metadata_and_independent_absence_roundtrip() {
    let source = pair(
        r#"<slur type="start" number="16" bezier-x="-0.04" bezier-y="7.125" orientation="over" placement="below"/>"#,
        r#"<slur type="stop" number="16" bezier-x="0" bezier-y="-123.001" orientation="under" placement="above"/>"#,
    );
    let imported = MusicXml.read(&source).unwrap();
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.authored_only" && d.effect == MappingEffect::Preserved)
    );
    let score = imported.value;
    let slur = &score.slurs[0];
    assert_eq!(slur.start.event, EventId("a".into()));
    assert_eq!(slur.end.event, EventId("b".into()));
    assert_eq!(slur.start.bezier_x, Some(r("-1/25")));
    assert_eq!(slur.start.bezier_y, Some(r("57/8")));
    assert_eq!(slur.start.orientation, Some(SlurOrientation::Over));
    assert_eq!(slur.start.placement, Some(Placement::Below));
    assert_eq!(slur.end.bezier_x, Some(r("0")));
    assert_eq!(slur.end.bezier_y, Some(r("-123001/1000")));
    assert_eq!(slur.end.orientation, Some(SlurOrientation::Under));
    assert_eq!(slur.end.placement, Some(Placement::Above));
    roundtrip(&score);
    for (start, end) in [
        (r#"<slur type="start"/>"#, r#"<slur type="stop"/>"#),
        (
            r#"<slur type="start" bezier-x="0"/>"#,
            r#"<slur type="stop" bezier-y="0"/>"#,
        ),
        (
            r#"<slur type="start" orientation="under"/>"#,
            r#"<slur type="stop" placement="above"/>"#,
        ),
    ] {
        let score = read(&pair(start, end));
        let out = roundtrip(&score);
        for field in ["bezier-x", "bezier-y", "orientation", "placement"] {
            assert_eq!(
                out.matches(field).count(),
                format!("{start}{end}").matches(field).count()
            );
        }
    }
}

#[test]
fn parallel_same_endpoint_slurs_keep_distinct_relation_ids_and_metadata() {
    let score = read(&pair(
        r#"<slur type="start" number="2"/><slur type="start" number="4" bezier-x="1.25"/>"#,
        r#"<slur type="stop" number="4" placement="below"/><slur type="stop" number="2"/>"#,
    ));
    assert_eq!(score.slurs.len(), 2);
    assert_ne!(score.slurs[0].id, score.slurs[1].id);
    assert!(
        score
            .slurs
            .iter()
            .all(|s| s.start.event.0 == "a" && s.end.event.0 == "b")
    );
    assert_eq!(
        score
            .slurs
            .iter()
            .filter(|s| s.start.bezier_x == Some(r("5/4")))
            .count(),
        1
    );
    let out = roundtrip(&score);
    assert_eq!(out.matches("<slur ").count(), 4);
}

#[test]
fn local_number_reuses_after_closing_across_voices() {
    let body = format!(
        "{}{}<backup><duration>2</duration></backup>{}{}",
        note("a", "1", 1, &mark("start", 1)),
        note("b", "1", 1, &mark("stop", 1)),
        note("c", "2", 2, &mark("start", 1)),
        note("d", "2", 2, &mark("stop", 1))
    );
    let score = read(&xml(&body));
    assert_eq!(score.slurs.len(), 2);
    assert_eq!(score.slurs[0].start.event.0, "a");
    assert_eq!(score.slurs[0].end.event.0, "b");
    assert_eq!(score.slurs[1].start.event.0, "c");
    assert_eq!(score.slurs[1].end.event.0, "d");
    let out = roundtrip(&score);
    assert_eq!(out.matches("<slur type=\"start\" number=\"1\"").count(), 2);
}

#[test]
fn endpoints_can_cross_voices_and_staves_with_stop_encountered_first() {
    let body = format!(
        "<forward><duration>1</duration></forward>{}<backup><duration>2</duration></backup>{}",
        note("b", "top", 2, &mark("stop", 3)),
        note("a", "bottom", 1, &mark("start", 3))
    );
    let score = read(&xml(&body));
    assert_eq!(score.slurs[0].start.event.0, "a");
    assert_eq!(score.slurs[0].end.event.0, "b");
    let out = roundtrip(&score);
    assert!(out.find("<slur type=\"stop\"").unwrap() < out.find("<slur type=\"start\"").unwrap());
    assert_eq!(score.parts[0].measures[0].events[0].offset, r("1"));
    assert_eq!(score.parts[0].measures[0].events[1].offset, r("0"));
}

#[test]
fn same_local_number_is_not_scoped_by_voice_or_staff() {
    let body = format!(
        "{}<backup><duration>1</duration></backup>{}{}",
        note("a", "1", 1, &mark("start", 1)),
        note("b", "2", 2, &mark("start", 1)),
        note("c", "2", 2, &mark("stop", 1))
    );
    error(&xml(&body), "slur.duplicate");
}

#[test]
fn stop_and_new_start_on_one_event_reuse_number_without_merging() {
    let score = read(&xml(&format!(
        "{}{}{}",
        note("a", "1", 1, &mark("start", 7)),
        note(
            "b",
            "1",
            1,
            &format!("{}{}", mark("stop", 7), mark("start", 7))
        ),
        note("c", "1", 1, &mark("stop", 7))
    )));
    assert_eq!(score.slurs.len(), 2);
    let out = roundtrip(&score);
    let middle = out
        .split("<note id=\"b\">")
        .nth(1)
        .unwrap()
        .split("</note>")
        .next()
        .unwrap();
    assert!(
        middle.contains("<slur type=\"stop\" number=\"1\"/><slur type=\"start\" number=\"1\"/>")
    );
    assert_eq!(score.parts[0].measures[0].events.len(), 3);
    assert!(
        score.parts[0].measures[0]
            .events
            .iter()
            .all(|e| e.duration == r("1"))
    );
}

#[test]
fn grace_to_grace_and_grace_to_principal_at_equal_onset_preserve_order_and_timing() {
    let source = xml(&format!(
        "{}{}{}",
        grace("g1", &format!("{}{}", mark("start", 1), mark("start", 2))),
        grace("g2", &mark("stop", 1)),
        note("main", "1", 1, &mark("stop", 2))
    ));
    let score = read(&source);
    assert_eq!(score.slurs.len(), 2);
    assert_eq!(
        score.graces.iter().map(|g| g.order).collect::<Vec<_>>(),
        [-2, -1]
    );
    assert_eq!(score.parts[0].measures[0].events.len(), 1);
    assert_eq!(score.parts[0].measures[0].events[0].offset, r("0"));
    assert_eq!(score.parts[0].measures[0].events[0].duration, r("1"));
    let out = roundtrip(&score);
    assert_eq!(out.matches("<duration>").count(), 1);
    assert!(out.find("id=\"g1\"").unwrap() < out.find("id=\"g2\"").unwrap());
    assert!(out.find("id=\"g2\"").unwrap() < out.find("id=\"main\"").unwrap());
    error(
        &source.replace(&mark("stop", 1), "<fermata/>"),
        "grace.profile",
    );
    error(
        &source.replace(&mark("stop", 1), "<tied type=\"stop\"/>"),
        "grace.profile",
    );
}

#[test]
fn chord_member_emission_order_and_cross_staff_graph_are_not_reordered() {
    let body = format!(
        "{}{}{}{}",
        note("z", "1", 1, &mark("start", 2)),
        note("a", "1", 2, &mark("start", 3)).replace(PITCH, &format!("<chord/>{PITCH}")),
        note("y", "1", 1, &mark("stop", 3)),
        note("b", "1", 2, &mark("stop", 2)).replace(PITCH, &format!("<chord/>{PITCH}"))
    );
    let score = read(&xml(&body));
    let out = roundtrip(&score);
    let again = read(&out);
    let ids = |s: &ScoreDocument| {
        s.parts[0].measures[0]
            .events
            .iter()
            .map(|e| e.id.0.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&score), ["z", "a", "y", "b"]);
    assert_eq!(ids(&again), ids(&score));
    assert_eq!(
        score.parts[0].measures[0].events,
        again.parts[0].measures[0].events
    );
}

#[test]
fn unmatched_duplicate_and_reversed_musical_endpoints_are_blocked() {
    error(&pair(&mark("start", 1), ""), "slur.unmatched");
    error(&pair("", &mark("stop", 1)), "slur.unmatched");
    error(&pair(&mark("start", 1), &mark("stop", 2)), "slur.unmatched");
    error(
        &pair(&mark("start", 1), &mark("start", 1)),
        "slur.duplicate",
    );
    error(&pair(&mark("stop", 1), &mark("stop", 1)), "slur.duplicate");
    error(&pair(&mark("stop", 1), &mark("start", 1)), "slur.order");
    error(
        &pair(&format!("{}{}", mark("start", 1), mark("stop", 1)), ""),
        "slur.anchor",
    );
    error(
        &pair(&mark("start", 1), &mark("stop", 1)).replacen(PITCH, "<rest/>", 1),
        "slur.anchor",
    );
    let equal = xml(&format!(
        "{}{}",
        note("a", "1", 1, &mark("start", 1)),
        note("b", "1", 1, &mark("stop", 1)).replace(PITCH, &format!("<chord/>{PITCH}"))
    ));
    error(&equal, "slur.order");
}

#[test]
fn unknown_fields_descendants_and_malformed_values_are_never_discarded() {
    for (start, feature) in [
        (r#"<slur type="continue"/>"#, "slur.type"),
        (r#"<slur/>"#, "slur.type"),
        (r#"<slur type="start" number="0"/>"#, "slur.number"),
        (r#"<slur type="start" number="17"/>"#, "slur.number"),
        (r#"<slur type="start" number="1.0"/>"#, "slur.number"),
        (
            r#"<slur type="start" orientation="above"/>"#,
            "slur.orientation",
        ),
        (
            r#"<slur type="start" placement="under"/>"#,
            "slur.placement",
        ),
        (r#"<slur type="start" bezier-x="1e2"/>"#, "number.decimal"),
        (r#"<slur type="start" bezier-y="NaN"/>"#, "number.decimal"),
        (r#"<slur type="start" bezier-x="1/4"/>"#, "number.decimal"),
        (
            r#"<slur type="start" line-type="solid"/>"#,
            "attribute.line-type",
        ),
        (
            r#"<slur type="start" default-x="3"/>"#,
            "attribute.default-x",
        ),
        (
            r#"<slur type="start" bezier-x2="3"/>"#,
            "attribute.bezier-x2",
        ),
        (
            r##"<slur type="start" color="#000000"/>"##,
            "attribute.color",
        ),
        (
            r#"<slur type="start"><slur type="stop"/></slur>"#,
            "element.slur",
        ),
        (r#"<slur type="start">ignored</slur>"#, "xml.content"),
    ] {
        error(&pair(start, &mark("stop", 1)), feature);
    }
    error(
        &pair(
            "<slur type=\"start\" bezier-x=\"\u{a0}1\"/>",
            &mark("stop", 1),
        ),
        "slur.number_whitespace",
    );
    error(
        &pair(
            &format!("<slur type=\"start\" bezier-x=\"{}\"/>", "1".repeat(65)),
            &mark("stop", 1),
        ),
        "number.precision",
    );
}

#[test]
fn export_rejects_nonterminating_or_overprecision_coordinates() {
    let original = read(&pair(&mark("start", 1), &mark("stop", 1)));
    for value in [r("1/3"), r("1/8589934592"), r(&"1".repeat(65))] {
        let mut score = original.clone();
        score.slurs[0].end.bezier_x = Some(value);
        let report = MusicXml.write(&score).unwrap_err();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.feature == "slur.coordinate_decimal"),
            "{report:?}"
        );
    }
}

#[test]
fn export_allocates_at_most_sixteen_open_numbers_in_actual_voice_order() {
    let mut body = String::new();
    for i in 0..17 {
        body.push_str(&note(&format!("s{i}"), "1", 1, &mark("start", 1)));
        body.push_str(&note(&format!("e{i}"), "1", 1, &mark("stop", 1)));
    }
    let mut score = read(&xml(&body));
    assert_eq!(score.slurs.len(), 17);
    assert!(score.slurs.windows(2).all(|pair| pair[0].id < pair[1].id));
    roundtrip(&score); // Seventeen disjoint source pairs are perfectly representable.
    let second = VoiceId("v1_2".into());
    score.parts[0].voices.push(ScoreVoice {
        id: second.clone(),
        staff: 1,
    });
    for event in &mut score.parts[0].measures[0].events {
        if event.id.0.starts_with('e') {
            event.voice = second.clone();
        }
    }
    // Musical spans still never overlap, but emitted voice one opens all 17.
    let report = MusicXml.write(&score).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.number_capacity"),
        "{report:?}"
    );
    score.slurs.pop();
    roundtrip(&score); // Exactly sixteen must work, without merging any relation.
}

#[test]
fn export_closes_stop_first_relation_before_opening_another_at_number_capacity() {
    let mut body = String::from("<forward><duration>6</duration></forward>");
    // Voice one encounters all sixteen stops at t6 before their starts.
    for number in 1..=16 {
        let mut endpoint = note(&format!("end{number}"), "1", 1, &mark("stop", number));
        if number > 1 {
            endpoint = endpoint.replace(PITCH, &format!("<chord/>{PITCH}"));
        }
        body.push_str(&endpoint);
    }
    body.push_str("<backup><duration>6</duration></backup>");
    // At t1, the start closes the first relation and the stop opens another.
    // Their reused local number is legal only in this encounter order.
    body.push_str(&note(
        "boundary",
        "2",
        1,
        &format!("{}{}", mark("start", 1), mark("stop", 1)),
    ));
    // The other fifteen starts follow as a chord at t2.
    for number in 2..=16 {
        let mut endpoint = note(&format!("start{number}"), "2", 1, &mark("start", number));
        if number > 2 {
            endpoint = endpoint.replace(PITCH, &format!("<chord/>{PITCH}"));
        }
        body.push_str(&endpoint);
    }
    body.push_str("<backup><duration>3</duration></backup>");
    body.push_str(&note("first", "3", 1, &mark("start", 1)));

    let score = read(&xml(&body));
    assert_eq!(score.slurs.len(), 17);
    let out = roundtrip(&score);
    let boundary = out
        .split("<note id=\"boundary\">")
        .nth(1)
        .unwrap()
        .split("</note>")
        .next()
        .unwrap();
    assert!(
        boundary.contains("<slur type=\"start\" number=\"1\"/><slur type=\"stop\" number=\"1\"/>")
    );
    assert_eq!(out.matches("<slur ").count(), 34);
    assert!(out.contains("<slur type=\"stop\" number=\"16\"/>"));
    assert!(!out.contains("<slur type=\"stop\" number=\"17\"/>"));
    assert!(!out.contains("<slur type=\"start\" number=\"17\"/>"));
}

#[test]
fn export_preserves_sixteen_parallel_endpoint_pairs_but_rejects_seventeen() {
    let mut score = read(&pair(&mark("start", 1), &mark("stop", 1)));
    for i in 1..16 {
        let mut slur = score.slurs[0].clone();
        slur.id = RelationId(format!("parallel{i}"));
        score.slurs.push(slur);
    }
    assert_eq!(roundtrip(&score).matches("<slur ").count(), 32);
    let mut slur = score.slurs[0].clone();
    slur.id = RelationId("overflow".into());
    score.slurs.push(slur);
    let report = MusicXml.write(&score).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.number_capacity"),
        "{report:?}"
    );
}

#[test]
fn number_scope_is_part_local_and_unmatched_parts_do_not_pair() {
    let source = pair(&mark("start", 1), &mark("stop", 1));
    let part = source
        .split("<part id=\"p\">")
        .nth(1)
        .unwrap()
        .split("</part>")
        .next()
        .unwrap();
    let second = format!("<part id=\"p2\">{part}</part>")
        .replace("id=\"m\"", "id=\"m2\"")
        .replace("id=\"a\"", "id=\"c\"")
        .replace("id=\"b\"", "id=\"d\"");
    let source = source
        .replace(
            "</part-list>",
            "<score-part id=\"p2\"><part-name>Second</part-name></score-part></part-list>",
        )
        .replace("</score-partwise>", &format!("{second}</score-partwise>"));
    let score = read(&source);
    assert_eq!(score.slurs.len(), 2);
    roundtrip(&score);
    let unmatched = source.replacen(&mark("stop", 1), "", 1);
    let last_start = unmatched.rfind(&mark("start", 1)).unwrap();
    let mut unmatched = unmatched;
    unmatched.replace_range(last_start..last_start + mark("start", 1).len(), "");
    error(
        &unmatched.replace("<notations></notations>", ""),
        "slur.unmatched",
    );
}
