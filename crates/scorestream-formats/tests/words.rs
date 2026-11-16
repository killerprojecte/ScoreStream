use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>2</divisions><staves>2</staves></attributes>{body}<note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note></measure></part></score-partwise>"#
    )
}
fn words(text: &str, attrs: &str, offset: &str) -> String {
    format!(
        r#"<direction placement="above" system="only-top"><direction-type><words{attrs}>{text}</words></direction-type><offset sound="no">{offset}</offset><staff>2</staff></direction>"#
    )
}
fn roundtrip(xml: &str) -> (scorestream_core::score::ScoreDocument, String) {
    let m = MusicXml.read(xml).unwrap().value;
    let s = SfsAdapter.write(&m).unwrap().value;
    assert_eq!(m, SfsAdapter.read(&s).unwrap().value);
    assert_eq!(
        m,
        MusicXml
            .read(&MusicXml.write(&m).unwrap().value)
            .unwrap()
            .value
    );
    (m, s)
}
fn unchanged(m: &scorestream_core::score::ScoreDocument, s: &str) {
    let mut plain = m.clone();
    plain.words.clear();
    let mut a = serde_json::to_value(scorestream_app::playback::plan_source(s).unwrap()).unwrap();
    let mut b = serde_json::to_value(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap(),
    )
    .unwrap();
    for k in ["diagnostics", "warnings"] {
        a.as_object_mut().unwrap().remove(k);
        b.as_object_mut().unwrap().remove(k);
    }
    assert_eq!(a, b);
}
#[test]
fn literal_authored_text_full_style_axes_and_order_roundtrip_without_sound_inference() {
    let text = "  Attacca\n cresc. &amp; \u{e522} "
        .replace(
            "\n", "
",
        )
        .replace("\u{e522}", "");
    let body = format!(
        "{}{}",
        words(
            &text,
            r#" font-family="Leland Text" font-size="12.00" font-weight="bold" font-style="italic" default-x="-37.68" default-y="0" relative-x="116.72" relative-y="-25""#,
            "8"
        ),
        words("pizz. arco a tempo", "", "1")
    );
    let (m, s) = roundtrip(&score(&body));
    assert_eq!(m.words.len(), 2);
    assert_eq!(
        m.words[0].words.text,
        "  Attacca
 cresc. &  "
    );
    assert_eq!(m.words[0].words.default_x, Some("-942/25".parse().unwrap()));
    assert_eq!(m.words[0].words.default_y, Some("0".parse().unwrap()));
    assert_eq!(m.words[1].words.default_y, None);
    assert_eq!(m.words[0].offset, "4".parse().unwrap());
    assert_eq!(m.words[1].offset, "1/2".parse().unwrap());
    assert!(m.dashes.is_empty());
    assert!(m.dynamics.is_empty());
    unchanged(&m, &s);
    let view = scorestream_app::Workspace::from_source(&s)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "words.authored_only")
    );
}
#[test]
fn signed_words_offsets_keep_explicit_tempo_and_numeric_sound_independent() {
    for flag in ["yes", "no"] {
        let note = r#"<note id="a"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>"#;
        let mixed = words("Allegro", "", "-2")
            .replace("<staff>2</staff>", "")
            .replace("sound=\"no\"", &format!("sound=\"{flag}\""))
            .replace(
                "</direction>",
                "<sound dynamics=\"60\" tempo=\"71\"/></direction>",
            );
        let (m, s) = roundtrip(
            &score(&format!("{note}{mixed}"))
                .replace("<duration>8</duration>", "<duration>6</duration>"),
        );
        assert_eq!(m.words[0].offset.to_string(), "0");
        assert_eq!(
            m.sound_dynamics[0].at.to_string(),
            if flag == "yes" { "0" } else { "1" }
        );
        assert_eq!(m.tempos.len(), 1);
        unchanged(&m, &s);
    }
}
#[test]
fn dashes_groups_remain_distinct_and_shared_text_does_not_broaden_old_wire() {
    let group = r#"<direction><direction-type><words font-family="Edwin" font-size="10" font-style="italic">cresc.</words></direction-type><direction-type><dashes type="start"/></direction-type></direction>"#;
    let (m, _) = roundtrip(&score(&format!("{}{}", words("dolce", "", "0"), group)));
    assert_eq!(m.words.len(), 1);
    assert_eq!(m.dashes.len(), 1);
    assert!(
        MusicXml
            .read(&score(
                &group.replace("font-style=\"italic\"", "font-weight=\"bold\"")
            ))
            .is_err()
    );
}
#[test]
fn complex_words_and_explicit_unsupported_sound_fail_instead_of_disappearing() {
    for bad in [
 words("x"," color=\"#FF0000\"","0"),words("x"," font-weight=\"heavy\"","0"),words("x"," font-size=\"0\"","0"),words("<b>cresc.</b>","","0"),
 words("x","","9"),words("pizz.","","0").replace("</direction>","<sound pizzicato=\"unknown\"/></direction>"),
 words("tempo","","0").replace("</direction-type>","</direction-type><direction-type><dynamics><p/></dynamics></direction-type>"),
 words("a","","0").replace("</direction-type>","</direction-type><direction-type><words>b</words></direction-type>")
 ] {assert!(MusicXml.read(&score(&bad)).is_err(),"{bad}");}
}
#[test]
fn native_words_require_closed_typed_fields_and_safe_xml_text() {
    let (m, _) = roundtrip(&score(&words("dolce", "", "0")));
    for mode in 0..6 {
        let mut doc = scorestream_formats::sfs::to_document(&m).unwrap().value;
        if mode == 0 {
            doc.required_extensions.clear();
        }
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m) if m.kind == scorestream_core::words::KIND => {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        use scorestream_core::Scalar;
        match mode {
            1 => {
                mark.attributes.remove("text");
            }
            2 => {
                mark.attributes
                    .insert("font_weight".into(), Scalar::String("heavy".into()));
            }
            3 => {
                mark.attributes.insert("index".into(), Scalar::Integer(4));
            }
            4 => {
                mark.attributes
                    .insert("default_x".into(), Scalar::String("1/3".into()));
            }
            5 => {
                mark.target = "#missing".into();
            }
            _ => {}
        }
        assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    }
    let mut bad = m;
    bad.words[0].words.text = "".into();
    assert!(MusicXml.write(&bad).is_err());
}

#[test]
fn ordered_same_type_words_are_not_flattened_or_interpreted() {
    let group = r#"<direction placement="below"><direction-type><words default-y="-40" relative-x="-3.92" relative-y="-34.16">sempre </words><words font-family="Leland Text"></words></direction-type></direction>"#;
    let (m, s) = roundtrip(&score(group));
    assert_eq!(m.words.len(), 1);
    assert_eq!(m.words[0].words.text, "sempre ");
    assert_eq!(
        m.words[0].words.relative_x.as_ref().unwrap().to_string(),
        "-98/25"
    );
    let rows = &m.words[0].continuations;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].text.chars().map(u32::from).collect::<Vec<_>>(),
        vec![0xe522, 0xe522]
    );
    assert_eq!(rows[0].font_family.as_deref(), Some("Leland Text"));
    assert_eq!(rows[0].default_y, None);
    let xml = MusicXml.write(&m).unwrap().value;
    assert_eq!(xml.matches("<direction-type>").count(), 1);
    assert_eq!(xml.matches("<words").count(), 2);
    unchanged(&m, &s);
    for bad in [
        group.replace("</direction>", "<sound dynamics=\"50\"/></direction>"),
        group.replace(
            "</direction-type>",
            "<dynamics><f/></dynamics></direction-type>",
        ),
        group.replace(
            "</direction-type>",
            "<symbol>dynamicForte</symbol></direction-type>",
        ),
        group.replace("font-family=\"Leland Text\"", "color=\"#ff0000\""),
    ] {
        assert!(MusicXml.read(&score(&bad)).is_err());
    }
}

#[test]
fn continuation_wire_requires_one_contiguous_group_with_matching_owner_and_onset() {
    let group = "<direction><direction-type><words>a</words><words>b</words><words>c</words></direction-type></direction>";
    let (m, _) = roundtrip(&score(group));
    let mut reordered = scorestream_formats::sfs::to_document(&m).unwrap().value;
    for frame in &mut reordered.frames {
        frame.records.reverse();
    }
    assert_eq!(
        scorestream_core::words::decode(&reordered).unwrap(),
        m.words
    );
    for mode in 0..6 {
        let mut doc = scorestream_formats::sfs::to_document(&m).unwrap().value;
        let records = &mut doc.frames[0].records;
        let pos=records.iter().position(|r|matches!(r,scorestream_core::Record::Mark(m) if m.attributes.contains_key("row"))).unwrap();
        if mode == 0 {
            records.remove(pos);
        } else if mode == 1 {
            records.push(records[pos].clone());
        } else {
            let scorestream_core::Record::Mark(mark) = &mut records[pos] else {
                unreachable!()
            };
            match mode {
                2 => {
                    mark.attributes
                        .insert("row".into(), scorestream_core::Scalar::Integer(0));
                }
                3 => {
                    mark.attributes
                        .insert("index".into(), scorestream_core::Scalar::Integer(1));
                }
                4 => {
                    mark.attributes.insert(
                        "placement".into(),
                        scorestream_core::Scalar::String("above".into()),
                    );
                }
                _ => {
                    mark.target = "#missing".into();
                }
            }
        }
        assert!(scorestream_core::words::decode(&doc).is_err(), "{mode}");
    }
}
