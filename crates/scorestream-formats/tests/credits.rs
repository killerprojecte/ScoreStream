use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn fixture(credits: &str) -> String {
    format!(
        r#"<score-partwise version="4.0">{credits}<part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#
    )
}
const CREDITS: &str = r#"<credit page="1"><credit-type>title</credit-type><credit-words default-x="-1.2500" default-y="+0.00" justify="center" valign="top" font-size="22">  Title &amp; &lt;literal&gt;
第二行  </credit-words></credit><credit><credit-words> </credit-words></credit><credit page="2"><credit-type>rights</credit-type><credit-words default-y=".5" justify="right" valign="bottom" font-size="9.5">Rights</credit-words></credit>"#;
#[test]
fn credits_roundtrip_preserves_order_exact_text_and_presence_without_sound_changes() {
    let score = MusicXml.read(&fixture(CREDITS)).unwrap().value;
    assert_eq!(score.credits.len(), 3);
    assert_eq!(score.credits[0].text, "  Title & <literal>\n第二行  ");
    assert_eq!(
        score.credits[0].default_x.as_ref().unwrap().to_string(),
        "-5/4"
    );
    assert_eq!(
        score.credits[0].default_y.as_ref().unwrap().to_string(),
        "0"
    );
    assert_eq!(score.credits[1].page, None);
    assert_eq!(score.credits[1].default_y, None);
    assert_eq!(score.credits[1].text, " ");
    let sfs = SfsAdapter.write(&score).unwrap().value;
    let reopened = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(score, reopened);
    assert_eq!(sfs, SfsAdapter.write(&reopened).unwrap().value);
    let xml = MusicXml.write(&reopened).unwrap().value;
    assert_eq!(score, MusicXml.read(&xml).unwrap().value);
    let plain = SfsAdapter
        .write(&MusicXml.read(&fixture("")).unwrap().value)
        .unwrap()
        .value;
    let view = scorestream_app::Workspace::from_source(&sfs)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "credits.preview_unavailable")
    );
    let a = scorestream_app::playback::plan_source(&sfs).unwrap();
    let b = scorestream_app::playback::plan_source(&plain).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}
#[test]
fn unsupported_credit_structure_and_invalid_values_are_not_discarded() {
    for credit in [
        r#"<credit><credit-image source="x"/></credit>"#,
        r#"<credit><credit-words>A</credit-words><credit-words>B</credit-words></credit>"#,
        r#"<credit><credit-words>A</credit-words><credit-type>title</credit-type></credit>"#,
        r#"<credit><credit-type>title</credit-type><credit-type>subtitle</credit-type><credit-words>A</credit-words></credit>"#,
        r#"<credit page="0"><credit-words>A</credit-words></credit>"#,
        r#"<credit page="-1"><credit-words>A</credit-words></credit>"#,
        r#"<credit><credit-words justify="other">A</credit-words></credit>"#,
        r#"<credit><credit-words valign="other">A</credit-words></credit>"#,
        r#"<credit><credit-words font-size="0">A</credit-words></credit>"#,
        r#"<credit><credit-words default-x="1e3">A</credit-words></credit>"#,
        r#"<credit><credit-words default-y="NaN">A</credit-words></credit>"#,
        r#"<credit><credit-words relative-x="0">A</credit-words></credit>"#,
        r#"<credit><credit-words xml:lang="en">A</credit-words></credit>"#,
    ] {
        assert!(MusicXml.read(&fixture(credit)).is_err(), "{credit}");
    }
}
#[test]
fn malformed_wire_and_direct_model_are_rejected() {
    let score = MusicXml.read(&fixture(CREDITS)).unwrap().value;
    let original = scorestream_formats::sfs::to_document(&score).unwrap().value;
    for mode in 0..11 {
        let mut doc = original.clone();
        if mode == 0 {
            doc.required_extensions
                .retain(|e| e != scorestream_core::credits::EXTENSION);
        }
        if mode == 8 {
            doc.document_kind = scorestream_core::DocumentKind::Fragment;
        }
        if mode == 9 {
            doc.frames[0].time = scorestream_core::Rational::one();
        }
        if mode == 10 {
            doc.frames.iter_mut().for_each(|f| f.records.retain(|r| !matches!(r, scorestream_core::Record::Mark(m) if m.kind == scorestream_core::credits::KIND)));
            assert!(scorestream_formats::sfs::from_document(&doc).is_err());
            continue;
        }
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m) if m.kind == scorestream_core::credits::KIND => {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        use scorestream_core::Scalar;
        match mode {
            1 => {
                mark.target = "p".into();
            }
            2 => {
                mark.attributes.insert("index".into(), Scalar::Integer(1));
            }
            3 => {
                mark.attributes
                    .insert("default_x".into(), Scalar::String("1/3".into()));
            }
            4 => {
                mark.attributes.insert("page".into(), Scalar::Integer(0));
            }
            5 => {
                mark.attributes
                    .insert("justify".into(), Scalar::String("other".into()));
            }
            6 => {
                mark.attributes
                    .insert("extra".into(), Scalar::String("lost".into()));
            }
            7 => {
                mark.attributes.remove("text");
            }
            _ => {}
        }
        assert!(
            scorestream_formats::sfs::from_document(&doc).is_err(),
            "mode {mode}"
        );
    }
    let mut invalid = score.clone();
    invalid.credits[0].font_size = Some("-1".parse().unwrap());
    assert!(SfsAdapter.write(&invalid).is_err());
    assert!(MusicXml.write(&invalid).is_err());
    invalid = score;
    invalid.credits[0].text.push('\0');
    assert!(MusicXml.write(&invalid).is_err());
}
