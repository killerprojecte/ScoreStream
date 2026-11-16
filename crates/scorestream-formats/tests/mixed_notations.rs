use scorestream_core::{notation::RelationId, score::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXml, MusicXmlOptions},
    sfs::{self, SfsAdapter},
};

fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        articulations: ArticulationProfile::AuthoredMixedV1,
        ..Default::default()
    }
}
fn read(xml: &str) -> ScoreDocument {
    musicxml::read_with_options(xml, &options()).unwrap().value
}
fn source() -> String {
    include_str!("fixtures/mixed-authored-notations.musicxml").into()
}
fn roundtrip(xml: &str) -> (ScoreDocument, String) {
    let first = read(xml);
    let json = serde_json::to_string(&first).unwrap();
    assert_eq!(serde_json::from_str::<ScoreDocument>(&json).unwrap(), first);
    let native = SfsAdapter.write(&first).unwrap().value;
    assert!(native.contains("!requires scorestream.notations@1"));
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(again, first);
    let xml = MusicXml.write(&again).unwrap().value;
    let last = read(&xml);
    assert_eq!(last, first);
    assert_eq!(SfsAdapter.write(&last).unwrap().value, native);
    (first, xml)
}
#[test]
fn mixed_wrappers_order_and_existing_family_payloads_roundtrip_exactly() {
    let (model, xml) = roundtrip(&source());
    assert_eq!(model.ties.len(), 2);
    assert_eq!(model.slurs.len(), 2);
    assert_eq!(model.fermatas.len(), 1);
    let order = model.notation_order.as_ref().unwrap();
    assert_eq!(order.events.len(), 3);
    assert!(order.events[0].wrappers[1].is_empty());
    assert!(matches!(
        order.events[0].wrappers[0].as_slice(),
        [
            NotationChild::Slur { .. },
            NotationChild::Tied {
                phase: NotationPhase::Start
            },
            NotationChild::Articulations { group_index: 0 },
            NotationChild::Slur { .. }
        ]
    ));
    assert!(matches!(
        order.events[1].wrappers[0].as_slice(),
        [
            NotationChild::Articulations { group_index: 0 },
            NotationChild::Fermata {},
            NotationChild::Tied {
                phase: NotationPhase::Stop
            }
        ]
    ));
    assert_eq!(
        model.authored_articulations.as_ref().unwrap().events[0].notations[0].print_object,
        Some(false)
    );
    assert!(xml.contains("<notations id=\"empty\" print-object=\"yes\"></notations>"));
    assert_eq!(
        model.parts[0].measures[0].events.len(),
        3,
        "no sounding merge"
    );
    assert_eq!(
        model.fermatas[0].relative_y.as_ref().unwrap().to_string(),
        "-3/2"
    );
}
#[test]
fn old_profiles_remain_closed_and_pure_authored_payloads_do_not_change() {
    let xml = source();
    assert!(MusicXml.read(&xml).is_err());
    let strict = MusicXmlOptions {
        articulations: ArticulationProfile::AuthoredV1,
        ..Default::default()
    };
    assert!(musicxml::read_with_options(&xml, &strict).is_err());
    let pure = include_str!("fixtures/authored-articulation-family.musicxml");
    let old = musicxml::read_with_options(pure, &strict).unwrap().value;
    let new = read(pure);
    assert_eq!(old, new);
    assert!(new.notation_order.is_none());
}
#[test]
fn every_child_permutation_preserves_order_without_flattening_or_new_tie_meaning() {
    // Independent starts permit every local order, including repeated groups.
    let base = source();
    let marker = "<slur type=\"start\" number=\"1\" bezier-x=\"1.25\"/><tied type=\"start\" number=\"1\"/><articulations id=\"a-group\"><staccato/><staccato/></articulations><slur type=\"start\" number=\"2\" placement=\"below\"/>";
    let leaves = [
        "<slur type=\"start\" number=\"1\" bezier-x=\"1.25\"/>",
        "<tied type=\"start\" number=\"1\"/>",
        "<articulations id=\"a-group\"><staccato/><staccato/></articulations>",
        "<slur type=\"start\" number=\"2\" placement=\"below\"/>",
    ];
    assert!(base.contains(marker));
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let indices = [a, b, c, d];
                    if indices
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != 4
                    {
                        continue;
                    }
                    roundtrip(&base.replace(marker, &indices.map(|i| leaves[i]).concat()));
                }
            }
        }
    }
}
#[test]
fn public_model_missing_duplicate_foreign_or_reordered_references_reject() {
    let model = read(&source());
    let mut cases = Vec::new();
    let mut no_order = model.clone();
    no_order.notation_order = None;
    cases.push(no_order);
    let mut no_family = model.clone();
    no_family.authored_articulations = None;
    cases.push(no_family);
    let mut missing = model.clone();
    missing.notation_order.as_mut().unwrap().events[0].wrappers[0].remove(0);
    cases.push(missing);
    let mut duplicate = model.clone();
    let w = &mut duplicate.notation_order.as_mut().unwrap().events[0].wrappers[0];
    w.push(w[0].clone());
    cases.push(duplicate);
    let mut foreign = model.clone();
    foreign.notation_order.as_mut().unwrap().events[0].wrappers[0][0] = NotationChild::Slur {
        relation: RelationId("missing".into()),
        phase: NotationPhase::Start,
    };
    cases.push(foreign);
    let mut wrong_group = model.clone();
    wrong_group.notation_order.as_mut().unwrap().events[0].wrappers[0][2] =
        NotationChild::Articulations { group_index: 4 };
    cases.push(wrong_group);
    let mut wrong_host = model.clone();
    wrong_host.notation_order.as_mut().unwrap().events[0]
        .event
        .0 = "c".into();
    cases.push(wrong_host);
    let mut tie_order = model.clone();
    let e = &mut tie_order.notation_order.as_mut().unwrap().events[1];
    e.wrappers[0][2] = NotationChild::Tied {
        phase: NotationPhase::Start,
    };
    e.wrappers[1][0] = NotationChild::Tied {
        phase: NotationPhase::Stop,
    };
    cases.push(tie_order);
    for bad in cases {
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
}
#[test]
fn unknown_json_versions_fields_and_inapplicable_child_data_reject() {
    let value = serde_json::to_value(read(&source())).unwrap();
    for (field, data) in [
        ("version", serde_json::json!("2")),
        ("foreign", serde_json::json!(true)),
    ] {
        let mut bad = value.clone();
        bad["notation_order"][field] = data;
        assert!(serde_json::from_value::<ScoreDocument>(bad).is_err());
    }
    let mut bad = value;
    bad["notation_order"]["events"][0]["wrappers"][0][0]["group_index"] = serde_json::json!(0);
    assert!(serde_json::from_value::<ScoreDocument>(bad).is_err());
}
#[test]
fn reserved_capability_and_records_never_silently_enable_playback() {
    let model = read(&source());
    let native = SfsAdapter.write(&model).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&native).is_err());
    let doc = sfs::to_document(&model).unwrap().value;
    assert!(scorestream_core::notation::lower_document(&doc).is_err());
    for source in [
        native.replace("!requires scorestream.notations@1\n", ""),
        native.replace("scorestream.notations@1", "scorestream.notations@2"),
        native
            .lines()
            .filter(|line| !line.contains("scorestream.notations."))
            .collect::<Vec<_>>()
            .join("\n"),
    ] {
        assert!(SfsAdapter.read(&source).is_err());
        assert!(scorestream_app::playback::plan_source(&source).is_err());
    }
}
#[test]
fn notation_is_not_sounding_continuity_or_a_repair_for_unsupported_ties() {
    let base = source();
    for xml in [
        base.replacen("<tie type=\"start\"/>", "", 1),
        base.replacen("<tied type=\"start\" number=\"1\"/>", "", 1),
        base.replacen(
            "<tie type=\"start\"/>",
            "<tie type=\"start\" time-only=\"1\"/>",
            1,
        ),
        base.replacen(
            "<tied type=\"start\" number=\"1\"/>",
            "<tied type=\"let-ring\"/>",
            1,
        ),
        base.replacen(
            "<tied type=\"start\" number=\"1\"/>",
            "<tied type=\"continue\"/>",
            1,
        ),
        base.replacen(
            "<tied type=\"start\" number=\"1\"/>",
            "<tied type=\"start\" placement=\"above\"/>",
            1,
        ),
        base.replacen("<slur type=\"start\"", "<slur type=\"continue\"", 1),
        base.replace(
            "<fermata type=\"inverted\" relative-y=\"-1.5\">angled</fermata>",
            "<fermata/><fermata/>",
        ),
    ] {
        assert!(
            musicxml::read_with_options(&xml, &options()).is_err(),
            "{xml}"
        );
    }
}
#[test]
fn wrapper_source_ids_never_capture_generated_slur_identity() {
    let xml = source().replace("id=\"a-wrapper\"", "id=\"mx_slur1\"");
    let model = read(&xml);
    assert!(model.slurs.iter().all(|s| s.id.0 != "mx_slur1"));
    assert!(
        model
            .notation_order
            .as_ref()
            .unwrap()
            .events
            .iter()
            .flat_map(|e| &e.wrappers)
            .flatten()
            .filter_map(|c| if let NotationChild::Slur { relation, .. } = c {
                Some(relation)
            } else {
                None
            })
            .all(|r| model.slurs.iter().any(|s| &s.id == r))
    );
    let native = SfsAdapter.write(&model).unwrap().value;
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(again, model);
    assert!(MusicXml.write(&again).is_ok());
}

#[test]
fn independent_host_association_order_is_native_exact_and_xml_reports_normalization() {
    let mut model = read(&source());
    model.notation_order.as_mut().unwrap().events.reverse();
    let native = SfsAdapter.write(&model).unwrap().value;
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(again.notation_order, model.notation_order);
    let output = MusicXml.write(&again).unwrap();
    assert!(
        output
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "notations.event_order")
    );
    assert_eq!(
        read(&output.value).notation_order,
        read(&source()).notation_order
    );
}

#[test]
fn actual_mixed_import_snapshot_keeps_notation_without_enabling_preview_or_sound() {
    let model = read(&source());
    let source = SfsAdapter.write(&model).unwrap().value;
    let workspace = scorestream_app::Workspace::from_source(&source).unwrap();
    let actual = serde_json::to_value(workspace.current()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/tests/fixtures/mixed-authored-notations.json");
    if std::env::var_os("UPDATE_MIXED_NOTATION_FIXTURE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let checked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        actual, checked,
        "Fixture must come from actual XML -> common -> SFS -> Workspace pipeline"
    );
    assert!(scorestream_app::playback::plan_source(&source).is_err());
}

#[test]
fn chord_members_staffs_and_cross_voice_slurs_keep_independent_hosts() {
    let mut xml = source().replace(
        "<divisions>1</divisions>",
        "<divisions>1</divisions><staves>2</staves>",
    );
    xml = xml.replace(
        "<notations id=\"a-wrapper\"",
        "<notations><slur type=\"start\" number=\"3\"/></notations><notations id=\"a-wrapper\"",
    );
    xml=xml.replace("    <note id=\"b\">", r#"    <note id="chord"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>2</staff><notations><fermata/><articulations><accent/></articulations></notations></note>
    <note id="b">"#);
    xml=xml.replace("  </measure>",r#"    <backup><duration>3</duration></backup><forward><duration>2</duration><voice>2</voice><staff>2</staff></forward>
    <note id="other"><pitch><step>G</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff><notations><articulations><tenuto/></articulations><slur type="stop" number="3"/></notations></note>
  </measure>"#);
    let model = read(&xml);
    let native = SfsAdapter.write(&model).unwrap().value;
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(again.notation_order, model.notation_order);
    assert_eq!(again.slurs, model.slurs);
    let out = MusicXml.write(&again).unwrap().value;
    let third = read(&out);
    assert_eq!(third.notation_order, model.notation_order);
    assert_eq!(third.slurs, model.slurs);
    let chord = third.parts[0].measures[0]
        .events
        .iter()
        .find(|e| e.id.0 == "chord")
        .unwrap();
    assert_eq!(chord.staff, 2);
    assert!(chord.chord.is_some());
    assert_eq!(
        third
            .fermatas
            .iter()
            .filter(|f| f.event.0 == "chord")
            .count(),
        1
    );
    assert_eq!(
        third.fermatas.iter().filter(|f| f.event.0 == "a").count(),
        0
    );
    assert!(
        third
            .slurs
            .iter()
            .any(|s| s.start.event.0 == "a" && s.end.event.0 == "other")
    );
}

fn slur_study(a: &str, b: &str, c: &str) -> String {
    let mut xml = source();
    // Retain the explicit three pitched hosts; replace only this synthetic test's
    // notation/tie material, never external corpus input.
    for (id, children) in [("a", a), ("b", b), ("c", c)] {
        let start = xml.find(&format!("<note id=\"{id}\">")).unwrap();
        let end = start + xml[start..].find("</note>").unwrap() + "</note>".len();
        let note = format!(
            "<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations>{children}<articulations/></notations></note>"
        );
        xml.replace_range(start..end, &note);
    }
    xml
}
#[test]
fn same_host_slur_open_before_close_never_reuses_an_active_number() {
    for middle in [
        "<slur type=\"start\" number=\"2\"/><slur type=\"stop\" number=\"1\"/>",
        "<slur type=\"stop\" number=\"1\"/><slur type=\"start\" number=\"2\"/>",
    ] {
        roundtrip(&slur_study(
            "<slur type=\"start\" number=\"1\"/>",
            middle,
            "<slur type=\"stop\" number=\"2\"/>",
        ));
    }
}
#[test]
fn slur_capacity_uses_authored_child_order_not_a_convenient_sorted_order() {
    let starts = (1..=16)
        .map(|n| format!("<slur type=\"start\" number=\"{n}\"/>"))
        .collect::<String>();
    let stops = (1..=16)
        .map(|n| format!("<slur type=\"stop\" number=\"{n}\"/>"))
        .collect::<String>();
    let xml = slur_study(
        &starts,
        "<slur type=\"stop\" number=\"1\"/><slur type=\"start\" number=\"1\"/>",
        &stops,
    );
    let (mut model, _) = roundtrip(&xml);
    let b = &mut model.notation_order.as_mut().unwrap().events[1];
    b.wrappers[0].swap(0, 1); // Legal typed order, but requires 17 live XML numbers.
    assert!(
        SfsAdapter.write(&model).is_ok(),
        "native relation identities have no 16-number limit"
    );
    let report = MusicXml.write(&model).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.number_capacity")
    );
}
#[test]
fn mixed_cross_voice_stop_first_encounters_do_not_reverse_endpoints() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="inst"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><forward><duration>2</duration><voice>1</voice></forward><note id="late"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><articulations/><slur type="stop" number="1"/></notations></note><backup><duration>3</duration></backup><note id="early"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><notations><slur type="start" number="1"/><articulations/></notations></note><forward><duration>2</duration><voice>2</voice></forward></measure></part></score-partwise>"#;
    let model = read(xml);
    assert_eq!(model.slurs[0].start.event.0, "early");
    assert_eq!(model.slurs[0].end.event.0, "late");
    let native = SfsAdapter.write(&model).unwrap().value;
    let second = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(second.notation_order, model.notation_order);
    let out = MusicXml.write(&second).unwrap().value;
    assert_eq!(read(&out).notation_order, model.notation_order);
    assert_eq!(read(&out).slurs, model.slurs);
}

#[test]
fn native_tie_chain_continue_expands_only_to_ordinary_stop_then_start_not_layout_continue() {
    let mut model = read(&source());
    model.ties = vec![ScoreTie {
        id: RelationId("chain".into()),
        events: vec![
            scorestream_core::notation::EventId("a".into()),
            scorestream_core::notation::EventId("b".into()),
            scorestream_core::notation::EventId("c".into()),
        ],
    }];
    let native = SfsAdapter.write(&model).unwrap().value;
    assert!(native.contains("~ tie chain continue=#b"));
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(again.ties, model.ties);
    assert_eq!(again.notation_order, model.notation_order);
    let out = MusicXml.write(&again).unwrap().value;
    assert!(!out.contains("type=\"continue\""));
    let imported = read(&out);
    assert_eq!(imported.notation_order, read(&source()).notation_order);
    assert_eq!(imported.parts[0].measures[0].events.len(), 3);
    assert_eq!(
        imported.ties.len(),
        2,
        "XML graph is retained; chain grouping normalization is reported separately"
    );
}
