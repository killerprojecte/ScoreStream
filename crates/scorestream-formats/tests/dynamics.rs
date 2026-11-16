use scorestream_core::{
    Rational,
    notation::{DynamicSign, Placement},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a}!={b}");
}
fn note(id: &str, dur: &str) -> String {
    format!(
        "<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>{dur}</duration><voice>1</voice></note>"
    )
}
fn score(body: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\"><measure id=\"m\" number=\"1\"><attributes><divisions>4</divisions><staves>2</staves><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"
    )
}
fn mark(symbol: &str, offset: &str, staff: &str, sound: &str) -> String {
    format!(
        "<direction placement=\"below\"><direction-type><dynamics><{symbol}/></dynamics></direction-type>{offset}{staff}{sound}</direction>"
    )
}
fn full(body: &str) -> String {
    score(&format!("{body}{}", note("n", "16")))
}
fn read(xml: &str) -> scorestream_core::score::ScoreDocument {
    MusicXml.read(xml).unwrap().value
}
fn plan(model: &scorestream_core::score::ScoreDocument) -> scorestream_app::playback::PlaybackPlan {
    scorestream_app::playback::plan_source(&SfsAdapter.write(model).unwrap().value).unwrap()
}
fn error(xml: &str, feature: &str) {
    let e = MusicXml.read(xml).unwrap_err();
    assert!(
        e.diagnostics.iter().any(|d| d.feature == feature),
        "{feature}: {e:?}"
    );
}

#[test]
fn original_numeric_values_are_not_replaced_by_visible_label_policy() {
    for (symbol, percent, velocity) in [
        ("p", "44.44", 39.996),
        ("sf", "106.67", 96.003),
        ("ff", "106.67", 96.003),
        ("fp", "95.56", 86.004),
        ("ppp", "18.89", 17.001),
    ] {
        // Numeric/symbol pairs from originals, in a synthetic minimal score. Source layout is not discarded from an original document.
        let model = read(&full(&mark(
            symbol,
            "",
            "",
            &format!("<sound dynamics=\"{percent}\"/>"),
        )));
        assert!(!model.dynamics[0].sound);
        assert_eq!(model.dynamics[0].placement, Some(Placement::Below));
        let p = plan(&model);
        let n = &p.notes[0];
        close(f64::from(n.velocity) * n.gain_envelope[0].gain, velocity);
        assert!(
            n.gain_envelope
                .iter()
                .all(|pt| (f64::from(n.velocity) * pt.gain - velocity).abs() < 1e-10)
        );
    }
}
#[test]
fn exact_common_and_sfs_storage_survive_roundtrips() {
    let model = read(&full(&mark("sf", "", "", "<sound dynamics=\"106.67\"/>")));
    assert_eq!(model.sound_dynamics[0].percent, r("10667/100"));
    assert_eq!(model.dynamics[0].sign, DynamicSign::Sforzato);
    let text = SfsAdapter.write(&model).unwrap().value;
    assert!(text.contains("sound_dynamics=10667/100"));
    assert!(text.contains("sound=false value=sf") || text.contains("value=sf"));
    let again = SfsAdapter.read(&text).unwrap().value;
    assert_eq!(model.dynamics, again.dynamics);
    assert_eq!(model.sound_dynamics, again.sound_dynamics);
    let xml = MusicXml.write(&again).unwrap().value;
    assert!(xml.contains("<sf/>"));
    assert!(!xml.contains("<sfz/>"));
    assert!(xml.contains("dynamics=\"106.67\""));
    let again = read(&xml);
    assert_eq!(model.dynamics, again.dynamics);
    assert_eq!(model.sound_dynamics, again.sound_dynamics);
}
#[test]
fn sf_and_sfz_written_authorial_spellings_remain_distinct() {
    let a = read(&full(&mark("sf", "", "", "")));
    let b = read(&full(&mark("sfz", "", "", "")));
    assert_ne!(a.dynamics[0].sign, b.dynamics[0].sign);
    assert_eq!(plan(&a).notes[0].velocity, 80);
    assert_eq!(plan(&b).notes[0].velocity, 80);
}
#[test]
fn absent_numeric_initial_state_remains_unassigned_through_roundtrips() {
    let model = read(&score(&format!(
        "{}{}{}",
        note("a", "4"),
        mark("p", "", "", "<sound dynamics=\"44.44\"/>"),
        note("b", "12")
    )));
    assert_eq!(model.sound_dynamics.len(), 1);
    assert_eq!(model.sound_dynamics[0].percent, r("1111/25"));
    assert_eq!(model.sound_dynamics[0].at, r("1"));
    let sfs = SfsAdapter.write(&model).unwrap().value;
    let sfs_model = SfsAdapter.read(&sfs).unwrap().value;
    let xml = MusicXml.write(&sfs_model).unwrap();
    assert!(
        xml.report
            .diagnostics
            .iter()
            .any(|d| d.feature == "sound_dynamics.initial_unassigned")
    );
    let again = MusicXml.read(&xml.value).unwrap();
    assert!(
        again
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "sound_dynamics.initial_unassigned")
    );
    assert_eq!(again.value.sound_dynamics, model.sound_dynamics);
    assert_eq!(again.value.dynamics, model.dynamics);
    assert_eq!(plan(&again.value).notes, plan(&model).notes);
    let p = plan(&model);
    assert_eq!(p.notes[0].velocity, 80);
    close(
        f64::from(p.notes[1].velocity) * p.notes[1].gain_envelope[0].gain,
        39.996,
    );
}
#[test]
fn display_and_sound_offset_layers_are_independent_and_exact() {
    for (sound_flag, sound_at) in [("", "1"), (" sound=\"no\"", "1"), (" sound=\"yes\"", "3/2")] {
        let body = format!(
            "{}{}{}",
            note("a", "4"),
            mark(
                "p",
                &format!("<offset{sound_flag}>2</offset>"),
                "",
                "<sound dynamics=\"44.44\"/>"
            ),
            note("b", "12")
        );
        let model = read(&score(&body));
        assert_eq!(model.dynamics[0].at, r("3/2"));
        assert_eq!(model.sound_dynamics[0].at, r(sound_at));
    }
    let body = format!(
        "{}{}{}",
        note("a", "4"),
        mark(
            "p",
            "<offset sound=\"yes\">2</offset>",
            "",
            "<sound dynamics=\"44.44\"><offset>-1</offset></sound>"
        ),
        note("b", "12")
    );
    let model = read(&score(&body));
    assert_eq!(model.dynamics[0].at, r("3/2"));
    assert_eq!(model.sound_dynamics[0].at, r("3/4"));
}
#[test]
fn sound_dynamics_and_tempo_share_one_sound_without_unit_confusion() {
    let model = read(&full("<sound tempo=\"60.25\" dynamics=\"44.44\"/>"));
    assert_eq!(model.tempos[0].quarter_bpm, r("241/4"));
    assert_eq!(model.sound_dynamics[0].percent, r("1111/25"));
    let p = plan(&model);
    close(p.duration_seconds, 960.0 / 241.0);
    close(
        f64::from(p.notes[0].velocity) * p.notes[0].gain_envelope[0].gain,
        39.996,
    );
}
#[test]
fn held_numeric_updates_are_steps_not_invented_crescendos() {
    let body = format!(
        "<sound dynamics=\"44.44\"/>{}<sound dynamics=\"106.67\"><offset>-8</offset></sound>",
        note("held", "16")
    );
    let model = read(&score(&body));
    let p = plan(&model);
    let n = &p.notes[0];
    let at = n
        .gain_envelope
        .iter()
        .filter(|v| v.offset_seconds == 1.0)
        .collect::<Vec<_>>();
    assert_eq!(at.len(), 2);
    close(at[0].gain, 39.996 / 40.0);
    close(at[1].gain, 96.003 / 40.0);
    close(n.duration_seconds, 1.84);
}
#[test]
fn zero_suggestion_is_silence_and_not_midi_note_off() {
    let p = plan(&read(&full("<sound dynamics=\"0\"/>")));
    assert_eq!(p.notes[0].velocity, 1);
    assert!(p.notes[0].gain_envelope.iter().all(|g| g.gain == 0.0));
}
#[test]
fn staff_scope_survives_equal_position_xml_and_sfs_roundtrip() {
    let model = read(&full(&mark(
        "p",
        "",
        "<staff>2</staff>",
        "<sound dynamics=\"44.44\"/>",
    )));
    assert_eq!(model.dynamics[0].scope.staff, Some(2));
    assert_eq!(model.sound_dynamics.len(), 1);
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert!(sfs.contains("= p.s2 sound_dynamics=1111/25"));
    let again = read(&MusicXml.write(&model).unwrap().value);
    assert_eq!(model.dynamics, again.dynamics);
    assert_eq!(model.sound_dynamics, again.sound_dynamics);
    assert_eq!(
        plan(&model).notes[0].velocity,
        80,
        "staff2 does not leak to staff1"
    );
}
#[test]
fn unmatched_staff_export_is_explicitly_unsupported_not_fake_display() {
    let model = read(&full(&mark(
        "p",
        "<offset>2</offset>",
        "<staff>2</staff>",
        "<sound dynamics=\"44.44\"/>",
    )));
    assert!(SfsAdapter.write(&model).is_ok());
    let report = MusicXml.write(&model).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "sound_dynamics.staff_export")
    );
}
#[test]
fn duplicate_controls_must_agree_exactly() {
    assert_eq!(
        read(&full(
            "<sound dynamics=\"44.44\"/><sound dynamics=\"44.440\"/>"
        ))
        .sound_dynamics
        .len(),
        1
    );
    error(
        &full("<sound dynamics=\"44.44\"/><sound dynamics=\"44.45\"/>"),
        "sound_dynamics.conflict",
    );
}
#[test]
fn source_higher_numeric_range_is_preserved_but_unrealizable_plan_rejects() {
    let model = read(&full("<sound dynamics=\"200\"/>"));
    assert_eq!(model.sound_dynamics[0].percent, r("200"));
    let text = SfsAdapter.write(&model).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&text).is_err());
    assert!(MusicXml.write(&model).is_ok());
}
#[test]
fn unsupported_words_glyphs_compounds_layout_wedges_and_sound_properties_block() {
    for (element, feature) in [
        ("<wedge type=\"continue\"/>", "wedge.type"),
        (
            "<words font-family=\"Leland Text\"><b>\u{e522}</b></words>",
            "element.b",
        ),
        (
            "<dynamics><other-dynamics font-family=\"Custom\">sf</other-dynamics><f/></dynamics>",
            "attribute.font-family",
        ),
        (
            "<dynamics font-family=\"Leland\"><p/></dynamics>",
            "attribute.font-family",
        ),
    ] {
        let source = full(&format!(
            "<direction><direction-type>{element}</direction-type><sound dynamics=\"44.44\"/></direction>"
        ));
        error(&source, feature);
    }
    error(
        &full(
            "<direction><direction-type><dynamics><p/><f/></dynamics></direction-type></direction>",
        ),
        "dynamics.composite",
    );
    error(
        &full("<sound dynamics=\"44.44\" time-only=\"2\"/>"),
        "attribute.time-only",
    );
    error(
        &full("<sound dynamics=\"44.44\" damper-pedal=\"yes\"/>"),
        "attribute.damper-pedal",
    );
    error(
        &full(&mark("p", "<offset>16</offset>", "", "")),
        "dynamics.measure_boundary",
    );
    error(&full("<sound dynamics=\"-1\"/>"), "sound_dynamics.percent");
}
#[test]
fn numeric_model_and_nested_display_json_remain_strict() {
    let model = read(&full(&mark("sf", "", "", "<sound dynamics=\"44.44\"/>")));
    for field in ["scope", "sign"] {
        let mut json = serde_json::to_value(&model).unwrap();
        json["dynamics"][0][field]["unknown"] = serde_json::json!("authorial");
        assert!(serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err());
    }
    let mut copy = model.clone();
    copy.sound_dynamics[0].percent = r("-1");
    assert!(SfsAdapter.write(&copy).is_err());
    let mut copy = model.clone();
    copy.sound_dynamics[0].at = r("4");
    assert!(MusicXml.write(&copy).is_err());
    let mut copy = model.clone();
    copy.sound_dynamics.push(copy.sound_dynamics[0].clone());
    assert!(SfsAdapter.write(&copy).is_err());
}
#[test]
fn sounding_symbolic_policy_is_retained_in_sfs_but_not_falsely_exported_as_numeric() {
    let mut model = read(&full(&mark("p", "", "", "")));
    model.sound_dynamics.clear();
    model.dynamics[0].sound = true;
    let text = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(
        scorestream_app::playback::plan_source(&text).unwrap().notes[0].velocity,
        48
    );
    assert!(
        MusicXml
            .write(&model)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "dynamics.symbolic_sound")
    );
}
#[test]
fn exact_fractional_non_decimal_percent_is_allowed_in_sfs_not_rounded_to_xml() {
    let mut model = read(&full("<sound dynamics=\"44.44\"/>"));
    model.sound_dynamics[0].percent = r("100/3");
    let text = SfsAdapter.write(&model).unwrap().value;
    close(
        scorestream_app::playback::plan_source(&text).unwrap().notes[0].gain_envelope[0].gain,
        1.0,
    );
    assert!(MusicXml.write(&model).is_err());
}

#[test]
fn display_only_and_unrelated_parts_preserve_existing_playback() {
    let base = read(&full(""));
    let mut display = read(&full(&mark("p", "", "", "")));
    assert!(display.sound_dynamics.is_empty());
    assert_eq!(plan(&display).notes, plan(&base).notes);
    // A second independent part must not acquire an unrequested baseline.
    let mut other = base.parts[0].clone();
    other.id.0 = "other".into();
    other.instrument_metadata.as_mut().unwrap().source_id = "other_instrument".into();
    other.voices[0].id.0 = "other_voice".into();
    other.measures[0].id = "other_measure".into();
    other.measures[0].events[0].id.0 = "other_note".into();
    other.measures[0].events[0].voice = other.voices[0].id.clone();
    display.parts.push(other);
    let xml = MusicXml.write(&display).unwrap();
    let again = read(&xml.value);
    assert!(again.sound_dynamics.is_empty());
    assert!(plan(&again).notes.iter().all(|n| n.velocity == 80));
    let sfs_model = SfsAdapter
        .read(&SfsAdapter.write(&again).unwrap().value)
        .unwrap()
        .value;
    assert!(sfs_model.sound_dynamics.is_empty());
    assert_eq!(sfs_model.dynamics, again.dynamics);
    assert_eq!(plan(&sfs_model).notes, plan(&again).notes);
    // Explicit numeric state in the first part cannot leak into the other.
    display
        .sound_dynamics
        .push(scorestream_core::score::ScoreSoundDynamics {
            at: r("0"),
            scope: display.dynamics[0].scope.clone(),
            percent: r("50"),
        });
    let p = plan(&read(&MusicXml.write(&display).unwrap().value));
    assert_eq!(p.notes.iter().filter(|n| n.velocity == 45).count(), 1);
    assert_eq!(p.notes.iter().filter(|n| n.velocity == 80).count(), 1);
}

#[test]
fn numeric_dynamics_use_performed_navigation_not_linear_playback() {
    let xml = include_str!("fixtures/navigation-closed.musicxml").replacen(
        "<attributes>",
        "<sound dynamics=\"44.44\"/><attributes>",
        1,
    );
    let model = read(&xml);
    assert!(!model.barlines.is_empty());
    assert_eq!(model.sound_dynamics.len(), 1);
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert!(sfs.contains("!requires scorestream.navigation@1"));
    assert!(sfs.contains("sound_dynamics=1111/25"));
    let p = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(
        p.event_positions
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "a", "c", "d"]
    );
    assert_eq!(
        p.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [40, 40, 40, 40, 40]
    );
}

#[test]
fn authored_dynamic_coordinates_roundtrip_without_touching_sound() {
    let plain = full(&mark(
        "sf",
        "",
        "<staff>1</staff>",
        "<sound dynamics=\"106.67\"/>",
    ));
    let positioned = plain.replace("<dynamics>", "<dynamics default-x=\"-36.800\" default-y=\"+0.0\" relative-x=\".125\" relative-y=\"-40\">");
    let model = read(&positioned);
    let d = &model.dynamics[0];
    assert_eq!(d.default_x, Some(r("-184/5")));
    assert_eq!(d.default_y, Some(r("0")));
    assert_eq!(d.relative_x, Some(r("1/8")));
    assert_eq!(d.relative_y, Some(r("-40")));
    assert_eq!(model.sound_dynamics, read(&plain).sound_dynamics);
    assert!(!d.sound);
    let sfs = SfsAdapter.write(&model).unwrap().value;
    let reopened = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(model, reopened);
    assert_eq!(sfs, SfsAdapter.write(&reopened).unwrap().value);
    let xml = MusicXml.write(&reopened).unwrap().value;
    assert_eq!(model, read(&xml));
    let audible = |model: &scorestream_core::score::ScoreDocument| {
        let mut value = serde_json::to_value(plan(model)).unwrap();
        // Only provenance changes when authored source attributes change.
        // Keep diagnostic code/effect/policy/message as well as every audio field.
        for diagnostic in value["diagnostics"].as_array_mut().unwrap() {
            diagnostic.as_object_mut().unwrap().remove("object_id");
            diagnostic.as_object_mut().unwrap().remove("source");
        }
        value
    };
    assert_eq!(audible(&model), audible(&read(&plain)));
    assert!(
        scorestream_app::Workspace::from_source(&sfs)
            .unwrap()
            .current()
            .valid
    );
    let missing = read(&plain);
    assert!(missing.dynamics[0].default_y.is_none());
    // Every optional field, including relative coordinates, remains independently absent.
    for key in ["default-x", "default-y", "relative-x", "relative-y"] {
        let source = plain.replace("<dynamics>", &format!("<dynamics {key}=\"0\">"));
        let one = read(&source);
        assert_eq!(
            one,
            SfsAdapter
                .read(&SfsAdapter.write(&one).unwrap().value)
                .unwrap()
                .value
        );
        assert_eq!(one, read(&MusicXml.write(&one).unwrap().value));
    }
}

#[test]
fn dynamic_geometry_invalid_values_and_composite_symbols_remain_blocking() {
    let plain = full(&mark("f", "", "", ""));
    for key in ["default-x", "default-y", "relative-x", "relative-y"] {
        for value in ["", "NaN", "Infinity", "1e3", "1/3", "1..2", "&#160;1"] {
            assert!(
                MusicXml
                    .read(&plain.replace("<dynamics>", &format!("<dynamics {key}=\"{value}\">")))
                    .is_err(),
                "{key}={value}"
            );
        }
    }
    for value in [
        "<f/><p/>",
        "<other-dynamics font-family=\"Custom\">sf</other-dynamics><f/>",
        "<unknown/>",
    ] {
        assert!(
            MusicXml
                .read(&plain.replace(
                    "<dynamics><f/>",
                    &format!("<dynamics default-x=\"1\">{value}")
                ))
                .is_err()
        );
    }
    let mut model = read(&plain);
    model.dynamics[0].relative_y = Some(r("1/3"));
    assert!(SfsAdapter.write(&model).is_err());
    assert!(MusicXml.write(&model).is_err());
    let valid = SfsAdapter.write(&read(&plain)).unwrap().value;
    // Reject invalid direct SFS AST geometry, not merely XML values.
    let mut doc = scorestream_codec::parse(&valid).unwrap();
    for frame in &mut doc.frames {
        for record in &mut frame.records {
            if let scorestream_core::Record::Mark(m) = record {
                if m.kind == "dynamic" {
                    m.attributes.insert(
                        "relative_y".into(),
                        scorestream_core::Scalar::String("1/3".into()),
                    );
                }
            }
        }
    }
    assert!(scorestream_formats::sfs::from_document(&doc).is_err());
}

#[test]
fn ordered_plain_text_qualifiers_preserve_authored_intent_without_inventing_performance() {
    use scorestream_core::score::DynamicQualifierPosition::{After, Before};
    for (text, position) in [
        ("sempre ", Before),
        ("sempre più ", Before),
        ("più ", Before),
        ("dimin. ", Before),
        (" dolce", After),
        ("\tsempre\n ", Before),
    ] {
        let other = format!("<other-dynamics>{text}</other-dynamics>");
        let children = if position == Before {
            format!("{other}<p/>")
        } else {
            format!("<p/>{other}")
        };
        let plain = full(&mark(
            "p",
            "",
            "<staff>1</staff>",
            "<sound dynamics=\"44.44\"/>",
        ));
        let source = plain.replace(
            "<dynamics><p/>",
            &format!("<dynamics default-y=\"-36.8\" relative-y=\"0\">{children}"),
        );
        let model = read(&source);
        let q = model.dynamics[0].qualifier.as_ref().unwrap();
        assert_eq!(q.text, text);
        assert_eq!(q.position, position);
        assert_eq!(model.sound_dynamics, read(&plain).sound_dynamics);
        let sfs = SfsAdapter.write(&model).unwrap().value;
        let restored = SfsAdapter.read(&sfs).unwrap().value;
        let document = scorestream_codec::parse(&sfs).unwrap();
        let lowered = scorestream_core::notation::lower_document(&document).unwrap();
        assert!(lowered.directions.iter().any(|d| matches!(&d.kind, scorestream_core::notation::DirectionKind::Dynamic { qualifier: Some(value), sound: false, .. } if value == q)));
        let mut unchecked = lowered.clone();
        for direction in &mut unchecked.directions {
            if let scorestream_core::notation::DirectionKind::Dynamic {
                sound,
                qualifier: Some(_),
                ..
            } = &mut direction.kind
            {
                *sound = true;
            }
        }
        assert!(
            scorestream_core::notation::resolve_expression(&unchecked)
                .unwrap_err()
                .iter()
                .any(|d| d.code == "dynamics.qualifier_performance_unsupported")
        );
        assert_eq!(model, restored);
        assert_eq!(sfs, SfsAdapter.write(&restored).unwrap().value);
        let xml = MusicXml.write(&restored).unwrap().value;
        assert_eq!(read(&xml), model);
        let view = scorestream_app::Workspace::from_source(&sfs)
            .unwrap()
            .current();
        assert!(view.valid);
        assert!(
            view.diagnostics
                .iter()
                .any(|d| d.code == "dynamics.qualifier_unrealized")
        );
        let mut actual =
            serde_json::to_value(scorestream_app::playback::plan_source(&sfs).unwrap()).unwrap();
        assert!(
            actual["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "dynamic_qualifier_not_realized")
        );
        let mut expected = serde_json::to_value(plan(&read(&plain))).unwrap();
        actual.as_object_mut().unwrap().remove("diagnostics");
        actual.as_object_mut().unwrap().remove("warnings");
        expected.as_object_mut().unwrap().remove("diagnostics");
        expected.as_object_mut().unwrap().remove("warnings");
        assert_eq!(actual, expected);
        let mut sounding = model.clone();
        sounding.dynamics[0].sound = true;
        let sounding_sfs = SfsAdapter.write(&sounding).unwrap().value;
        assert!(
            scorestream_app::playback::plan_source(&sounding_sfs)
                .unwrap_err()
                .contains("dynamics.qualifier_performance_unsupported")
        );
        let absent_sound = sounding_sfs.replace(" sound=true", "");
        assert!(
            scorestream_app::playback::plan_source(&absent_sound)
                .unwrap_err()
                .contains("dynamics.qualifier_performance_unsupported")
        );
        assert!(
            scorestream_app::playback::plan_source(&SfsAdapter.write(&read(&plain)).unwrap().value)
                .is_ok()
        );
    }
}

#[test]
fn unsupported_qualifier_topology_and_incomplete_wire_fail_closed() {
    let plain = full(&mark("p", "", "", ""));
    for children in [
        "<p/><other-dynamics>a</other-dynamics><other-dynamics>b</other-dynamics>",
        "<p/><f/><other-dynamics>dolce</other-dynamics>",
        "<other-dynamics>dolce</other-dynamics>",
        "<p/><other-dynamics><words>dolce</words></other-dynamics>",
        "<p/><other-dynamics font-family=\"Leland\">dolce</other-dynamics>",
        "<p/><other-dynamics>  </other-dynamics>",
        "<p/><other-dynamics/>",
    ] {
        assert!(
            MusicXml.read(&plain.replace("<p/>", children)).is_err(),
            "{children}"
        );
    }
    let model = read(&plain.replace("<p/>", "<p/><other-dynamics> dolce</other-dynamics>"));
    for mode in 0..4 {
        let mut doc = scorestream_formats::sfs::to_document(&model).unwrap().value;
        for frame in &mut doc.frames {
            for record in &mut frame.records {
                if let scorestream_core::Record::Mark(m) = record {
                    if m.kind == "dynamic" {
                        match mode {
                            0 => {
                                m.attributes.remove("qualifier_text");
                            }
                            1 => {
                                m.attributes.remove("qualifier_position");
                            }
                            2 => {
                                m.attributes.insert(
                                    "qualifier_position".into(),
                                    scorestream_core::Scalar::String("inside".into()),
                                );
                            }
                            _ => {
                                m.attributes.insert(
                                    "qualifier_text".into(),
                                    scorestream_core::Scalar::String("  ".into()),
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    }
    let mut invalid = model.clone();
    invalid.dynamics[0].qualifier.as_mut().unwrap().text.clear();
    assert!(SfsAdapter.write(&invalid).is_err());
    invalid = model;
    invalid.dynamics[0]
        .qualifier
        .as_mut()
        .unwrap()
        .text
        .push('\0');
    assert!(MusicXml.write(&invalid).is_err());
}
