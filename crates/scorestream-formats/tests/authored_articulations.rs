use scorestream_core::{articulation_model::*, notation::Placement, score::ScoreDocument};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};

fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        articulations: ArticulationProfile::AuthoredV1,
        ..Default::default()
    }
}
fn source(notations: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="inst"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note id="e"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type>{notations}</note></measure></part></score-partwise>"#
    )
}
fn group(items: &str) -> String {
    source(&format!(
        "<notations><articulations>{items}</articulations></notations>"
    ))
}
fn read(xml: &str) -> ScoreDocument {
    musicxml::read_with_options(xml, &options()).unwrap().value
}
fn authored(model: &ScoreDocument) -> &AuthoredArticulations {
    model.authored_articulations.as_ref().unwrap()
}
fn item(model: &ScoreDocument) -> &AuthoredArticulation {
    &authored(model).events[0].notations[0].groups[0].items[0]
}
fn roundtrip(xml: &str) -> ScoreDocument {
    let first = read(xml);
    let json = serde_json::to_string(&first).unwrap();
    let decoded: ScoreDocument = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, first);
    let sfs = SfsAdapter.write(&decoded).unwrap().value;
    assert!(sfs.contains("!requires scorestream.articulation@1"));
    assert!(!sfs.contains("<articulations"));
    let common = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(common.authored_articulations, first.authored_articulations);
    let exported = MusicXml.write(&common).unwrap().value;
    let again = read(&exported);
    assert_eq!(again.authored_articulations, first.authored_articulations);
    assert_eq!(SfsAdapter.write(&again).unwrap().value, sfs);
    again
}
fn rejects(xml: &str, feature: &str) {
    let err = musicxml::read_with_options(xml, &options()).unwrap_err();
    assert!(
        err.diagnostics
            .iter()
            .any(|d| d.feature == feature && d.source.is_some()),
        "Expected {feature}, got {err:?}"
    );
}

#[test]
fn every_defined_choice_and_passive_other_roundtrips_as_distinct_typed_data() {
    let choices = [
        "accent",
        "strong-accent",
        "staccato",
        "tenuto",
        "detached-legato",
        "staccatissimo",
        "spiccato",
        "scoop",
        "plop",
        "doit",
        "falloff",
        "breath-mark",
        "caesura",
        "stress",
        "unstress",
        "soft-accent",
        "other-articulation",
    ];
    for name in choices {
        for placement in ["", " placement=\"above\"", " placement=\"below\""] {
            let model = roundtrip(&group(&format!("<{name}{placement}/>")));
            assert!(
                model.articulations.is_empty(),
                "Authored v1 is not legacy sound policy"
            );
            assert_eq!(authored(&model).version, ArticulationVersion::V1);
            let actual = item(&model);
            assert_eq!(actual.sign.name(), name);
            assert_eq!(
                actual.placement,
                match placement {
                    "" => None,
                    " placement=\"above\"" => Some(Placement::Above),
                    _ => Some(Placement::Below),
                }
            );
            assert_eq!(actual.print_style, ArticulationPrintStyle::default());
            assert_eq!(authored(&model).events[0].event.0, "e");
        }
    }
}
#[test]
fn order_repeated_empty_groups_wrappers_and_source_identity_survive_without_flattening() {
    let xml = source(
        r#"<notations id="wrapper_a" print-object="yes"><articulations id="empty"/><articulations id="stack"><tenuto placement="above"/><accent/><tenuto placement="below"/><strong-accent type="down" placement="above"/></articulations></notations><notations id="empty_wrapper" print-object="no"/><notations><articulations><staccato/></articulations></notations>"#,
    );
    let score = roundtrip(&xml);
    let event = &authored(&score).events[0];
    assert_eq!(event.notations.len(), 3);
    assert_eq!(event.notations[0].source_id.as_deref(), Some("wrapper_a"));
    assert_eq!(event.notations[0].print_object, Some(true));
    assert_eq!(event.notations[1].print_object, Some(false));
    assert_eq!(event.notations[2].print_object, None);
    assert!(event.notations[0].groups[0].items.is_empty());
    assert!(event.notations[1].groups.is_empty());
    let stack = &event.notations[0].groups[1].items;
    assert_eq!(
        stack.iter().map(|x| x.sign.name()).collect::<Vec<_>>(),
        vec!["tenuto", "accent", "tenuto", "strong-accent"]
    );
    assert_eq!(stack[0].placement, Some(Placement::Above));
    assert_eq!(stack[2].placement, Some(Placement::Below));
    assert_eq!(
        stack[3].sign,
        ArticulationSign::StrongAccent {
            orientation: Some(ArticulationOrientation::Down)
        }
    );
    assert_eq!(stack[3].placement, Some(Placement::Above));
    roundtrip(&source("<notations><articulations/></notations>"));
}
#[test]
fn exact_print_style_and_transition_fields_apply_to_every_choice_without_inferred_defaults() {
    let common = r##"default-x="-1.25" default-y="0" relative-x=".125" relative-y="100.00001" font-family="  Bravura,   serif  " font-style="italic" font-size="-0.5" font-weight="bold" color="#80AABBCC" placement="below""##;
    for name in [
        "accent",
        "strong-accent",
        "staccato",
        "tenuto",
        "detached-legato",
        "staccatissimo",
        "spiccato",
        "scoop",
        "plop",
        "doit",
        "falloff",
        "breath-mark",
        "caesura",
        "stress",
        "unstress",
        "soft-accent",
        "other-articulation",
    ] {
        let model = roundtrip(&group(&format!("<{name} {common}/>")));
        let s = &item(&model).print_style;
        assert_eq!(s.default_x.as_ref().unwrap().to_string(), "-5/4");
        assert_eq!(s.default_y.as_ref().unwrap().to_string(), "0");
        assert_eq!(s.relative_x.as_ref().unwrap().to_string(), "1/8");
        assert_eq!(
            s.relative_y.as_ref().unwrap().to_string(),
            "10000001/100000"
        );
        assert_eq!(s.font_family.as_deref(), Some("Bravura, serif"));
        assert_eq!(
            s.font_size,
            Some(ArticulationFontSize::Points("-1/2".parse().unwrap()))
        );
        assert_eq!(s.font_style, Some(ArticulationFontStyle::Italic));
        assert_eq!(s.font_weight, Some(ArticulationFontWeight::Bold));
        assert_eq!(s.color.as_deref(), Some("#80AABBCC"));
    }
    for name in ["scoop", "plop", "doit", "falloff"] {
        for kind in ["solid", "dashed", "dotted", "wavy"] {
            let model = roundtrip(&group(&format!(
                r#"<{name} line-shape="curved" line-type="{kind}" line-length="long" dash-length="-1.5" space-length="0"/>"#
            )));
            let line = item(&model).sign.line().unwrap();
            assert_eq!(line.shape, Some(ArticulationLineShape::Curved));
            assert_eq!(line.line_type.unwrap().name(), kind);
            assert_eq!(line.length, Some(ArticulationLineLength::Long));
            assert_eq!(line.dash_length.as_ref().unwrap().to_string(), "-3/2");
            assert_eq!(line.space_length.as_ref().unwrap().to_string(), "0");
        }
        let model = roundtrip(&group(&format!(r#"<{name} dash-length="2"/>"#)));
        assert_eq!(item(&model).sign.line().unwrap().line_type, None);
    }
    for size in [
        "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large",
    ] {
        let model = roundtrip(&group(&format!(
            r#"<accent font-size="{size}" font-style="normal" font-weight="normal"/>"#
        )));
        assert_eq!(
            item(&model).print_style.font_size,
            Some(ArticulationFontSize::Css(
                ArticulationCssFontSize::from_name(size).unwrap()
            ))
        );
    }
}
#[test]
fn orientation_breath_and_caesura_authored_empty_are_not_replaced_by_defaults() {
    for (attrs, expected) in [
        ("", None),
        (" type=\"up\"", Some(ArticulationOrientation::Up)),
        (" type=\"down\"", Some(ArticulationOrientation::Down)),
    ] {
        let model = roundtrip(&group(&format!(
            "<strong-accent{attrs} placement=\"below\"/>"
        )));
        assert_eq!(
            item(&model).sign,
            ArticulationSign::StrongAccent {
                orientation: expected
            }
        );
        assert_eq!(item(&model).placement, Some(Placement::Below));
    }
    for shape in ["", "comma", "tick", "upbow", "salzedo"] {
        let model = roundtrip(&group(&format!("<breath-mark>{shape}</breath-mark>")));
        assert_eq!(
            item(&model).sign,
            ArticulationSign::BreathMark {
                shape: BreathMarkShape::from_name(shape)
            }
        );
    }
    for shape in ["", "normal", "thick", "short", "curved", "single"] {
        let model = roundtrip(&group(&format!("<caesura>{shape}</caesura>")));
        assert_eq!(
            item(&model).sign,
            ArticulationSign::Caesura {
                shape: CaesuraShape::from_name(shape)
            }
        );
    }
    for (name, feature) in [
        ("breath-mark", "articulation.breath_shape"),
        ("caesura", "articulation.caesura_shape"),
    ] {
        for text in [" ", "\n", "normal ", " comma", "\u{a0}"] {
            rejects(&group(&format!("<{name}>{text}</{name}>")), feature);
        }
    }
}
#[test]
fn passive_other_text_is_not_xml_or_a_performance_instruction() {
    let model = roundtrip(&group(
        r#"<other-articulation smufl="articAccentAbove"> &#13;&#10;&#9;&lt;breath-mark/&gt; &amp; 私 </other-articulation>"#,
    ));
    assert_eq!(
        item(&model).sign,
        ArticulationSign::OtherArticulation {
            text: " \r\n\t<breath-mark/> & 私 ".into(),
            smufl: Some("articAccentAbove".into())
        }
    );
    let plain = roundtrip(&group("<other-articulation>  </other-articulation>"));
    assert_eq!(
        item(&plain).sign,
        ArticulationSign::OtherArticulation {
            text: "  ".into(),
            smufl: None
        }
    );
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&sfs).is_err());
    rejects(
        &group("<other-articulation><accent/></other-articulation>"),
        "element.accent",
    );
}
#[test]
fn invalid_attrs_unknown_values_bad_anchors_and_mixed_notations_fail_closed() {
    for (leaf, feature) in [
        ("<accent id=\"leaf\"/>", "attribute.id"),
        ("<accent type=\"up\"/>", "attribute.type"),
        ("<accent smufl=\"articAccentAbove\"/>", "attribute.smufl"),
        ("<strong-accent type=\"over\"/>", "articulation.type"),
        ("<scoop line-type=\"curly\"/>", "articulation.line-type"),
        ("<doit dash-length=\"1/3\"/>", "number.decimal"),
        ("<accent default-x=\"1e1\"/>", "number.decimal"),
        ("<accent default-x=\"\u{a0}1\"/>", "articulation.decimal"),
        (
            "<accent font-style=\"oblique\"/>",
            "articulation.font-style",
        ),
        ("<accent color=\"red\"/>", "articulation.color"),
        ("<accent color=\"#aabbcc\"/>", "articulation.color"),
        ("<accent font-family=\"a,,b\"/>", "articulation.font_family"),
        (
            "<other-articulation smufl=\"bad name\"/>",
            "articulation.smufl",
        ),
        ("<accent>\u{a0}</accent>", "articulation.content"),
        ("<sforzato/>", "element.sforzato"),
    ] {
        rejects(&group(leaf), feature);
    }
    rejects(
        &source("<notations print-object=\"maybe\"><articulations/></notations>"),
        "articulation.print-object",
    );
    rejects(
        &source("<notations><articulations/><fermata/></notations>"),
        "articulation.mixed_notations",
    );
    rejects(
        &source(
            "<notations><articulations/></notations><notations><slur type=\"start\"/></notations>",
        ),
        "articulation.mixed_notations",
    );
    rejects(
        &source("<notations id=\"unmapped\"><fermata/></notations>"),
        "articulation.wrapper_scope",
    );
    rejects(
        &group("<accent/>").replace("<pitch><step>C</step><octave>4</octave></pitch>", "<rest/>"),
        "articulation.event",
    );
    assert!(
        musicxml::read_with_options(
            &group("<accent/>").replace("<duration>1</duration>", "<grace/>"),
            &options()
        )
        .is_err()
    );
    assert!(musicxml::read_with_options(&group("<accent/>").replace("<pitch><step>C</step><octave>4</octave></pitch>","<unpitched><display-step>C</display-step><display-octave>4</display-octave></unpitched>"),&options()).is_err());
}
#[test]
fn source_ids_reserve_before_generated_hosts_and_reject_cross_domain_collisions() {
    let source=source("<notations id=\"mx_n1\"><articulations id=\"future\"><accent/></articulations></notations>").replace("<note id=\"e\">","<note>");
    let score = roundtrip(&source);
    assert_ne!(authored(&score).events[0].event.0, "mx_n1");
    assert_eq!(
        authored(&score).events[0].notations[0].source_id.as_deref(),
        Some("mx_n1")
    );
    for id in ["p", "m", "e", "shared"] {
        let xml = super_source_id(id);
        let err = musicxml::read_with_options(&xml, &options()).unwrap_err();
        assert!(
            err.diagnostics.iter().any(|d| d.feature == "xml.id"),
            "{err:?}"
        );
    }
    let mut score = read(&group("<accent/>"));
    score.authored_articulations.as_mut().unwrap().events[0].notations[0].source_id =
        Some("e".into());
    assert!(MusicXml.write(&score).is_err());
    assert!(SfsAdapter.write(&score).is_err());
}
fn super_source_id(id: &str) -> String {
    source(&format!(
        "<notations id=\"shared\"><articulations id=\"{id}\"><accent/></articulations></notations>"
    ))
}
#[test]
fn nonterminating_author_values_keep_native_exactness_but_musicxml_refuses_rounding() {
    let mut score = read(&group("<scoop/>"));
    score.authored_articulations.as_mut().unwrap().events[0].notations[0].groups[0].items[0]
        .print_style
        .default_x = Some("1/3".parse().unwrap());
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.decimal")
    );
    assert_eq!(
        SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value
            .authored_articulations,
        score.authored_articulations
    );
    for exact in [format!("1/1{}", "0".repeat(33)), "1".repeat(65)] {
        score.authored_articulations.as_mut().unwrap().events[0].notations[0].groups[0].items[0]
            .print_style
            .default_x = Some(exact.parse().unwrap());
        assert!(
            MusicXml
                .write(&score)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "articulation.decimal")
        );
        assert!(SfsAdapter.write(&score).is_ok());
    }
}
#[test]
fn legacy_default_four_signs_and_old_json_remain_compatible_while_v1_is_explicit() {
    let xml = group("<accent/><staccato/>");
    let legacy = MusicXml.read(&xml).unwrap().value;
    assert_eq!(legacy.articulations.len(), 2);
    assert!(legacy.authored_articulations.is_none());
    assert!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&legacy).unwrap().value).is_ok()
    );
    let new = roundtrip(&xml);
    assert!(new.articulations.is_empty());
    assert!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&new).unwrap().value).is_err()
    );
    assert!(MusicXml.read(&group("<strong-accent/>")).is_err());
    let mut json = serde_json::to_value(&new).unwrap();
    json["authored_articulations"]["version"] = "2".into();
    assert!(serde_json::from_value::<ScoreDocument>(json).is_err());
    let mut json = serde_json::to_value(&new).unwrap();
    json["authored_articulations"]["events"][0]["notations"][0]["groups"][0]["items"][0]["sign"]
        ["xml"] = "<accent/>".into();
    assert!(serde_json::from_value::<ScoreDocument>(json).is_err());
    let json = serde_json::to_string(&legacy).unwrap();
    assert!(!json.contains("authored_articulations"));
    assert_eq!(
        serde_json::from_str::<ScoreDocument>(&json).unwrap(),
        legacy
    );
    let old_options: MusicXmlOptions =
        serde_json::from_str(r#"{"irregular_measures":[]}"#).unwrap();
    assert_eq!(old_options.articulations, ArticulationProfile::LegacySimple);
}

#[test]
fn polyphonic_chord_members_and_second_staff_keep_independent_hosts() {
    let xml = include_str!("fixtures/articulation-duet.musicxml")
        .replace("<accent/>", "<strong-accent type=\"down\" placement=\"above\"/>")
        .replace("<tenuto placement=\"below\"/>", "<scoop line-shape=\"curved\" placement=\"below\"/>")
        .replace("<staff>2</staff></note>", "<staff>2</staff><notations id=\"bass_wrapper\"><articulations id=\"bass_group\"><breath-mark>tick</breath-mark><caesura>thick</caesura></articulations></notations></note>");
    let score = roundtrip(&xml);
    let a = authored(&score);
    assert_eq!(a.events.len(), 4);
    let head = a.events.iter().find(|a| a.event.0 == "head").unwrap();
    let member = a.events.iter().find(|a| a.event.0 == "member").unwrap();
    let bass = a.events.iter().find(|a| a.event.0 == "bass").unwrap();
    assert_eq!(head.notations[0].groups[0].items.len(), 2);
    assert_eq!(member.notations[0].groups[0].items.len(), 1);
    assert_eq!(member.notations[0].groups[0].items[0].sign.name(), "scoop");
    assert_eq!(
        bass.notations[0].groups[0]
            .items
            .iter()
            .map(|a| a.sign.name())
            .collect::<Vec<_>>(),
        vec!["breath-mark", "caesura"]
    );
    let events = &score.parts[0].measures[0].events;
    let event = |id: &str| events.iter().find(|e| e.id.0 == id).unwrap();
    assert_eq!(event("head").chord, event("member").chord);
    assert_ne!(event("head").id, event("member").id);
    assert_eq!(event("head").offset, event("member").offset);
    assert_eq!(event("head").offset, event("bass").offset);
    assert_ne!(event("head").voice, event("bass").voice);
    assert_eq!(event("bass").staff, 2);
}

#[test]
fn other_families_on_separate_events_roundtrip_but_each_performance_entry_refuses() {
    use scorestream_app::playback::{
        discover_source_fermatas, discover_source_graces, discover_source_graces_and_fermatas,
    };
    let rest = r#"<note id="rest"><rest/><duration>1</duration><voice>1</voice><type>quarter</type><notations><fermata/></notations></note>"#;
    let start = r#"<note id="start"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><slur type="start"/></notations></note>"#;
    let stop = r#"<note id="stop"><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><slur type="stop"/></notations></note>"#;
    let xml = group("<detached-legato/><soft-accent/>")
        .replace("<beats>1</beats>", "<beats>4</beats>")
        .replace("</measure>", &format!("{rest}{start}{stop}</measure>"));
    let score = roundtrip(&xml);
    assert_eq!(score.slurs.len(), 1);
    assert_eq!(score.fermatas.len(), 1);
    assert_eq!(authored(&score).events.len(), 1);
    let grace = r#"<note id="grace"><grace/><pitch><step>C</step><octave>4</octave></pitch><voice>1</voice><type>16th</type></note>"#;
    let xml = xml.replace(
        "<note id=\"start\">",
        &format!("{grace}<note id=\"start\">"),
    );
    let score = roundtrip(&xml);
    assert_eq!(score.graces.len(), 1);
    let source = SfsAdapter.write(&score).unwrap().value;
    let original = source.clone();
    let doc = scorestream_codec::parse(&source).unwrap();
    assert!(scorestream_core::notation::lower_document(&doc).is_err());
    assert!(scorestream_core::notation::perform_document(&doc, Default::default()).is_err());
    assert!(scorestream_app::playback::plan_source(&source).is_err());
    assert!(discover_source_fermatas(&source).is_err());
    assert!(discover_source_graces(&source).is_err());
    assert!(discover_source_graces_and_fermatas(&source).is_err());
    assert_eq!(source, original);
}

#[test]
fn grace_wrapper_identity_and_visibility_cannot_bypass_the_authored_scope_guard() {
    for wrapper in [
        r#"<notations id="grace_wrapper"/>"#,
        r#"<notations print-object="no"><slur type="start"/></notations>"#,
    ] {
        let grace = format!(
            r#"<note id="g"><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>16th</type>{wrapper}</note>"#
        );
        let xml = source("").replace("<note id=\"e\">", &format!("{grace}<note id=\"e\">"));
        rejects(&xml, "articulation.wrapper_scope");
    }
}

#[test]
fn unrelated_metadata_layout_and_notation_gates_remain_closed() {
    let xml = group("<strong-accent/>");
    // Synthetic boundary probe, NOT the untouched external MusicXML test suite.
    rejects(&xml.replace("<part-list>","<identification><miscellaneous><miscellaneous-field name=\"test\">kept</miscellaneous-field></miscellaneous></identification><part-list>"),"element.miscellaneous-field");
    rejects(
        &xml.replace("<measure id=\"m\"", "<measure width=\"100\" id=\"m\""),
        "attribute.width",
    );
    rejects(
        &xml.replace(
            "<strong-accent/>",
            "<strong-accent/><ornaments><trill-mark/></ornaments>",
        ),
        "element.ornaments",
    );
}

#[test]
fn actual_imported_family_snapshot_preserves_all_choices_for_fail_closed_preview() {
    let xml = include_str!("fixtures/authored-articulation-family.musicxml");
    let score = roundtrip(xml);
    let signs: Vec<_> = authored(&score)
        .events
        .iter()
        .flat_map(|e| &e.notations)
        .flat_map(|w| &w.groups)
        .flat_map(|g| &g.items)
        .map(|a| a.sign.name())
        .collect();
    assert_eq!(signs.len(), 17);
    let sfs = SfsAdapter.write(&score).unwrap().value;
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("articulation.performance_unsupported")
    );
    let workspace = scorestream_app::Workspace::from_source(&sfs).unwrap();
    let actual = serde_json::to_value(workspace.current()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/tests/fixtures/authored-articulation-family.json");
    if std::env::var_os("UPDATE_AUTHORED_ARTICULATION_FIXTURE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let checked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        actual, checked,
        "Regenerate only from the synthetic authored-family MusicXML through the real Rust pipeline"
    );
}

#[test]
fn underscored_xml_host_normalization_never_collides_with_authored_wrapper_identity() {
    let xml =
        source("<notations id=\"event1\"><articulations><accent/></articulations></notations>")
            .replace("<note id=\"e\">", "<note id=\"_e\">");
    let score = read(&xml);
    assert_eq!(authored(&score).events[0].event.0, "_e");
    let sfs = SfsAdapter.write(&score).unwrap();
    assert!(
        sfs.report
            .diagnostics
            .iter()
            .any(|d| d.feature == "identifier")
    );
    let again = SfsAdapter.read(&sfs.value).unwrap().value;
    assert_eq!(authored(&again).events[0].event.0, "event2");
    assert_eq!(
        authored(&again).events[0].notations[0].source_id.as_deref(),
        Some("event1")
    );
    let output = MusicXml.write(&again).unwrap().value;
    assert_eq!(
        read(&output).authored_articulations,
        again.authored_articulations
    );
}

#[test]
fn musicxml_reports_event_association_reordering_without_changing_local_authored_order() {
    let xml = include_str!("fixtures/articulation-duet.musicxml");
    let mut score = read(xml);
    score
        .authored_articulations
        .as_mut()
        .unwrap()
        .events
        .reverse();
    let native = SfsAdapter.write(&score).unwrap().value;
    assert_eq!(
        SfsAdapter
            .read(&native)
            .unwrap()
            .value
            .authored_articulations,
        score.authored_articulations
    );
    let output = MusicXml.write(&score).unwrap();
    assert!(
        output
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.event_order"
                && d.effect == scorestream_formats::MappingEffect::Normalized)
    );
    let after = read(&output.value);
    for event in &authored(&score).events {
        assert_eq!(
            authored(&after)
                .events
                .iter()
                .find(|e| e.event == event.event),
            Some(event)
        );
    }
}

#[test]
fn xml_generated_voice_chord_and_relation_names_never_steal_authored_source_ids() {
    let duet = include_str!("fixtures/articulation-duet.musicxml")
        .replacen("<notations>", "<notations id=\"v1_1\">", 1)
        .replacen("<articulations>", "<articulations id=\"c1_1\">", 1);
    let converted = musicxml::read_with_options(&duet, &options()).unwrap();
    assert!(
        converted
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.generated_identity")
    );
    let score = roundtrip(&duet);
    assert_eq!(score.parts[0].voices[0].id.0, "v1_1_");
    assert_eq!(
        score.parts[0].measures[0].events[0].chord.as_deref(),
        Some("c1_1_")
    );
    assert_eq!(
        authored(&score).events[0].notations[0].source_id.as_deref(),
        Some("v1_1")
    );
    assert_eq!(
        authored(&score).events[0].notations[0].groups[0]
            .source_id
            .as_deref(),
        Some("c1_1")
    );
    for family in ["slur", "tie"] {
        let id = if family == "slur" {
            "mx_slur1"
        } else {
            "mx_t1"
        };
        let xml = source(&format!(
            "<notations id=\"{id}\"><articulations><accent/></articulations></notations>"
        ));
        let extra=[("a","start"),("b","stop")].into_iter().map(|(id,phase)| {
            let sound=if family=="tie" {format!("<tie type=\"{phase}\"/>")} else {String::new()};
            let tag=if family=="tie" {"tied"} else {"slur"};
            format!("<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>{sound}<voice>1</voice><type>quarter</type><notations><{tag} type=\"{phase}\"/></notations></note>")
        }).collect::<String>();
        let xml = xml
            .replace("<beats>1</beats>", "<beats>3</beats>")
            .replace("</measure>", &format!("{extra}</measure>"));
        let score = roundtrip(&xml);
        let actual = if family == "tie" {
            &score.ties[0].id.0
        } else {
            &score.slurs[0].id.0
        };
        assert_eq!(actual, &format!("{id}_"));
        assert_eq!(
            authored(&score).events[0].notations[0].source_id.as_deref(),
            Some(id)
        );
    }
}
