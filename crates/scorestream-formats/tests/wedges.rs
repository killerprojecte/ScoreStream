use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>2</divisions><staves>2</staves><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}<note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#
    )
}
fn wedge(kind: &str, attrs: &str, offset: &str, staff: &str) -> String {
    format!(
        "<direction placement=\"below\"><direction-type><wedge type=\"{kind}\"{attrs}/></direction-type><offset sound=\"no\">{offset}</offset>{staff}</direction>"
    )
}
fn roundtrip(source: &str) -> (scorestream_core::score::ScoreDocument, String) {
    let model = MusicXml.read(source).unwrap().value;
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(model, SfsAdapter.read(&sfs).unwrap().value);
    assert_eq!(
        model,
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value
    );
    (model, sfs)
}
#[test]
fn ordered_endpoints_keep_geometry_scope_number_presence_and_end_boundary() {
    let body = format!(
        "{}{}{}{}",
        wedge(
            "diminuendo",
            " number=\"1\" default-y=\"-60.40\"",
            "1",
            "<staff>2</staff>"
        ),
        wedge(
            "stop",
            " number=\"1\" spread=\"11.50\"",
            "4",
            "<staff>2</staff>"
        ),
        wedge("crescendo", " default-y=\"0\" spread=\"0\"", "4", ""),
        wedge("stop", "", "8", "")
    );
    let (model, sfs) = roundtrip(&score(&body));
    assert_eq!(model.wedges.len(), 4);
    assert_eq!(model.wedges[0].offset, "1/2".parse().unwrap());
    assert_eq!(model.wedges[0].default_y, Some("-302/5".parse().unwrap()));
    assert_eq!(model.wedges[1].spread, Some("23/2".parse().unwrap()));
    assert_eq!(model.wedges[2].number, None);
    assert_eq!(model.wedges[3].offset, "4".parse().unwrap());
    assert_eq!(model.wedges[3].default_y, None);
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "wedge.authored_only")
    );
    let plain = scorestream_app::playback::plan_source(
        &SfsAdapter
            .write(&MusicXml.read(&score("")).unwrap().value)
            .unwrap()
            .value,
    )
    .unwrap();
    let mut a = serde_json::to_value(plan).unwrap();
    let mut b = serde_json::to_value(plain).unwrap();
    for key in ["diagnostics", "warnings"] {
        a.as_object_mut().unwrap().remove(key);
        b.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(a, b);
}
#[test]
fn original_orphan_stop_dangling_start_and_duplicate_start_remain_visible_not_repaired() {
    let body = format!(
        "{}{}{}",
        wedge("stop", " number=\"1\" spread=\"11.5\"", "0", ""),
        wedge("diminuendo", " number=\"1\" default-y=\"-70.83\"", "2", ""),
        wedge("diminuendo", " number=\"1\"", "4", "")
    );
    let (model, sfs) = roundtrip(&score(&body));
    assert_eq!(scorestream_core::wedges::unresolved_pairs(&model.wedges), 3);
    let view = scorestream_app::Workspace::from_source(&sfs)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "wedge.pairing_unresolved")
    );
    assert!(scorestream_app::playback::plan_source(&sfs).is_ok());
}
#[test]
fn unsupported_shape_and_invalid_native_endpoints_are_not_discarded() {
    for body in [
        wedge("continue", "", "0", ""),
        wedge("crescendo", " niente=\"yes\"", "0", ""),
        wedge("crescendo", " default-x=\"2\"", "0", ""),
        wedge("crescendo", " number=\"17\"", "0", ""),
        wedge("crescendo", " spread=\"NaN\"", "0", ""),
        wedge("crescendo", "", "9", ""),
        wedge("crescendo", "", "0", "<staff>3</staff>"),
    ] {
        assert!(MusicXml.read(&score(&body)).is_err(), "{body}");
    }
    let (model, _) = roundtrip(&score(&wedge("crescendo", "", "0", "")));
    for mode in 0..6 {
        let mut doc = scorestream_formats::sfs::to_document(&model).unwrap().value;
        if mode == 0 {
            doc.required_extensions.clear();
        }
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m) if m.kind == scorestream_core::wedges::KIND => {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        use scorestream_core::Scalar;
        match mode {
            1 => mark.target = "#missing".into(),
            2 => {
                mark.attributes.insert("index".into(), Scalar::Integer(3));
            }
            3 => {
                mark.attributes
                    .insert("part".into(), Scalar::String("other".into()));
            }
            4 => {
                mark.attributes
                    .insert("spread".into(), Scalar::String("1/3".into()));
            }
            5 => {
                mark.attributes
                    .insert("type".into(), Scalar::String("continue".into()));
            }
            _ => {}
        }
        assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    }
}
#[test]
fn signed_direction_offsets_and_numbered_overlap_do_not_move_numeric_sound() {
    let note = r#"<note id="a"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>"#;
    let mut source = score(&format!(
        "{note}{}{}<sound dynamics=\"60\"/>{}{}",
        wedge("crescendo", " number=\"1\"", "-2", ""),
        wedge("diminuendo", " number=\"2\"", "0", ""),
        wedge("stop", " number=\"1\" spread=\"11.5\"", "2", ""),
        wedge("stop", " number=\"2\"", "4", "")
    ));
    // Remaining three quarter notes complete the existing 4/4 measure.
    source = source
        .replace("<duration>8</duration>", "<duration>6</duration>")
        .replace("<type>whole</type>", "<type>half</type><dot/>");
    let (model, sfs) = roundtrip(&source);
    assert_eq!(
        model
            .wedges
            .iter()
            .map(|w| w.offset.to_string())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3"]
    );
    assert_eq!(scorestream_core::wedges::unresolved_pairs(&model.wedges), 0);
    let mut plain = model.clone();
    plain.wedges.clear();
    let mut a =
        serde_json::to_value(scorestream_app::playback::plan_source(&sfs).unwrap()).unwrap();
    let mut b = serde_json::to_value(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap(),
    )
    .unwrap();
    for key in ["diagnostics", "warnings"] {
        a.as_object_mut().unwrap().remove(key);
        b.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(a, b);
    assert_eq!(model.sound_dynamics.len(), 1);
}

#[test]
fn mixed_direction_sound_retains_independent_visual_and_sounding_positions() {
    for sound_flag in ["no", "yes"] {
        let note = r#"<note id="a"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>"#;
        let mixed = format!(
            r#"<direction><direction-type><wedge type="crescendo"/></direction-type><offset sound="{sound_flag}">-2</offset><sound dynamics="60"/></direction>"#
        );
        let source = score(&format!("{note}{mixed}"))
            .replace("<duration>8</duration>", "<duration>6</duration>")
            .replace("<type>whole</type>", "<type>half</type><dot/>");
        let (model, sfs) = roundtrip(&source);
        assert_eq!(model.wedges[0].offset.to_string(), "0");
        assert_eq!(model.sound_dynamics.len(), 1);
        let mut plain = model.clone();
        plain.wedges.clear();
        let mut a =
            serde_json::to_value(scorestream_app::playback::plan_source(&sfs).unwrap()).unwrap();
        let mut b = serde_json::to_value(
            scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                .unwrap(),
        )
        .unwrap();
        for key in ["diagnostics", "warnings"] {
            a.as_object_mut().unwrap().remove(key);
            b.as_object_mut().unwrap().remove(key);
        }
        assert_eq!(a, b);
        assert_eq!(
            model.sound_dynamics[0].at.to_string(),
            if sound_flag == "yes" { "0" } else { "1" }
        );
    }
}
