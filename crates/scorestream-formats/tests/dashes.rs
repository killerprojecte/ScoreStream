use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>2</divisions><staves>2</staves></attributes>{body}<note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note></measure></part></score-partwise>"#
    )
}
fn group(text: &str, style: &str, dash: &str, offset: &str) -> String {
    format!(
        r#"<direction placement="below"><direction-type><words{style}>{text}</words></direction-type><direction-type><dashes type="start"{dash}/></direction-type><offset sound="no">{offset}</offset></direction>"#
    )
}
fn stop() -> &'static str {
    r#"<direction placement="below"><direction-type><dashes type="stop" number="1"/></direction-type><offset sound="no">8</offset></direction>"#
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
    plain.dashes.clear();
    let mut a = serde_json::to_value(scorestream_app::playback::plan_source(s).unwrap()).unwrap();
    let mut b = serde_json::to_value(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap(),
    )
    .unwrap();
    for k in ["warnings", "diagnostics"] {
        a.as_object_mut().unwrap().remove(k);
        b.as_object_mut().unwrap().remove(k);
    }
    assert_eq!(a, b);
}
#[test]
fn ordered_styled_words_dashes_preserve_exact_values_and_sound() {
    let text = "  cresc.\n &amp; dim.  ".replace(
        "\n", "
",
    );
    let body = format!(
        "{}<sound dynamics=\"60\"/>{}",
        group(
            &text,
            r#" font-family="Edwin" font-size="10.00" font-style="italic" default-y="-86.99" relative-x="8.96""#,
            r#" number="1" default-y="-80" relative-x="0""#,
            "1"
        ),
        stop()
    );
    let (m, s) = roundtrip(&score(&body));
    let w = m.dashes[0].words.as_ref().unwrap();
    assert_eq!(
        w.text,
        "  cresc.
 & dim.  "
    );
    assert_eq!(w.default_y, Some("-8699/100".parse().unwrap()));
    assert_eq!(m.dashes[0].offset, "1/2".parse().unwrap());
    assert_eq!(m.dashes[0].relative_x, Some("0".parse().unwrap()));
    assert_eq!(m.dashes[1].words, None);
    assert_eq!(scorestream_core::dashes::unresolved_pairs(&m.dashes), 0);
    unchanged(&m, &s);
    let view = scorestream_app::Workspace::from_source(&s)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "dashes.authored_only")
    );
}
#[test]
fn system_only_top_ritard_is_preserved_not_interpreted_as_tempo() {
    let body = format!("{}{}", group("poco ritard.", "", "", "0"), stop()).replace(
        "placement=\"below\"",
        "placement=\"above\" system=\"only-top\"",
    );
    let (m, s) = roundtrip(&score(&body));
    assert_eq!(m.dashes[0].system.as_deref(), Some("only-top"));
    assert_eq!(m.dashes[0].number, None);
    assert_eq!(m.dashes[0].words.as_ref().unwrap().font_size, None);
    unchanged(&m, &s);
}
#[test]
fn orphan_and_dangling_groups_warn_without_repair_or_playback_block() {
    let (m, s) = roundtrip(&score(&format!(
        "{}{}",
        stop(),
        group("dim.", "", r#" number="1""#, "0")
    )));
    assert_eq!(scorestream_core::dashes::unresolved_pairs(&m.dashes), 2);
    unchanged(&m, &s);
    assert!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "dashes.pairing_unresolved")
    );
}
#[test]
fn unsupported_composites_styles_shapes_and_invalid_native_fields_fail_closed() {
    let valid = group("cresc.", "", "", "0");
    for bad in [
        valid.replace("start", "continue"),
        valid.replace("start", "stop"),
        valid.replace("<words>cresc.</words>", "<words><b>cresc.</b></words>"),
        valid.replace("<words>", "<words font-weight=\"bold\">"),
        valid.replace("<words>", "<words font-size=\"0\">"),
        valid.replace("<words>", "<words font-style=\"oblique\">"),
        valid.replace("<dashes ", "<dashes dash-length=\"3\" "),
        valid.replace(
            "<direction placement",
            "<direction system=\"anything\" placement",
        ),
        valid.replace("<offset sound=\"no\">0", "<offset sound=\"no\">9"),
        r#"<direction><direction-type><words>cresc.</words></direction-type><direction-type><words>more</words></direction-type></direction>"#.into(),
        r#"<direction><direction-type><dashes type="start"/></direction-type></direction>"#.into(),
    ] {
        assert!(MusicXml.read(&score(&bad)).is_err(), "{bad}");
    }
    let (m, _) = roundtrip(&score(&valid));
    for mode in 0..5 {
        let mut doc = scorestream_formats::sfs::to_document(&m).unwrap().value;
        if mode == 0 {
            doc.required_extensions.clear();
        }
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m) if m.kind == scorestream_core::dashes::KIND => {
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
                    .insert("font_size".into(), Scalar::String("1/3".into()));
            }
            3 => {
                mark.attributes.insert("index".into(), Scalar::Integer(3));
            }
            4 => {
                mark.target = "#missing".into();
            }
            _ => {}
        }
        assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    }
    let mut bad = m;
    bad.dashes[0].words.as_mut().unwrap().text = "bad\u{1}".replace("\u{1}", "");
    assert!(MusicXml.write(&bad).is_err());
}

#[test]
fn signed_group_offset_and_sound_flag_do_not_shift_numeric_cues() {
    for flag in ["yes", "no"] {
        let note = r#"<note id="a"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>"#;
        let mixed = group("cresc.", "", "", "-2")
            .replace("sound=\"no\"", &format!("sound=\"{flag}\""))
            .replace("</direction>", "<sound dynamics=\"60\"/></direction>");
        let source = score(&format!("{note}{mixed}"))
            .replace("<duration>8</duration>", "<duration>6</duration>");
        let (m, s) = roundtrip(&source);
        assert_eq!(m.dashes[0].offset.to_string(), "0");
        assert_eq!(
            m.sound_dynamics[0].at.to_string(),
            if flag == "yes" { "0" } else { "1" }
        );
        unchanged(&m, &s);
    }
}
