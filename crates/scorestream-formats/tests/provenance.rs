use scorestream_core::{
    Record, Scalar,
    provenance::{EncodingEntry, Provenance},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn xml(identification: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><identification>{identification}</identification><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#
    )
}
const ENCODING: &str = r#"<encoding><software> MuseScore &amp; Test&#13;&#10;二 </software><encoding-date>2026-07-11</encoding-date><supports type="yes" element="accidental"/><supports element="print" type="no" attribute="new-page" value=""/><software/><supports element="beam" type="yes"/><encoding-date>2000-02-29</encoding-date></encoding>"#;
#[test]
fn ordered_claims_strings_authors_and_optional_fields_roundtrip_canonically() {
    let input = xml(&format!(
        "<creator type=\"author\">Second</creator><creator type=\"author\">First</creator>{ENCODING}<miscellaneous/>"
    ));
    let score = MusicXml.read(&input).unwrap().value;
    assert_eq!(score.metadata.authors, vec!["Second", "First"]);
    let p = score.provenance.as_ref().unwrap();
    assert!(p.miscellaneous_empty);
    let entries = p.encoding.as_ref().unwrap();
    assert_eq!(entries.len(), 7);
    assert_eq!(
        entries[0],
        EncodingEntry::Software {
            text: " MuseScore & Test\r\n二 ".into()
        }
    );
    assert_eq!(
        entries[3],
        EncodingEntry::Supports {
            supported: false,
            element: "print".into(),
            attribute: Some("new-page".into()),
            value: Some("".into())
        }
    );
    assert_eq!(entries[4], EncodingEntry::Software { text: "".into() });
    let wire = SfsAdapter.write(&score).unwrap().value;
    let mut recovered = SfsAdapter.read(&wire).unwrap().value;
    recovered.metadata.source.clear();
    assert_eq!(score, recovered);
    assert_eq!(wire, SfsAdapter.write(&recovered).unwrap().value);
    let output = MusicXml.write(&recovered).unwrap().value;
    let again = MusicXml.read(&output).unwrap().value;
    assert_eq!(score, again);
    assert_eq!(output, MusicXml.write(&again).unwrap().value);
    let json = serde_json::to_value(&score).unwrap();
    assert_eq!(
        serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).unwrap(),
        score
    );
    let mut doc = scorestream_codec::parse(&wire).unwrap();
    for f in &mut doc.frames {
        f.records.reverse();
    }
    assert_eq!(
        scorestream_core::provenance::decode(&doc).unwrap(),
        score.provenance
    );
}
#[test]
fn absent_and_explicit_empty_containers_stay_distinct() {
    for (body, encoding, misc) in [
        ("", None, false),
        ("<encoding/>", Some(vec![]), false),
        ("<miscellaneous/>", None, true),
        ("<encoding/><miscellaneous/>", Some(vec![]), true),
    ] {
        let score = MusicXml.read(&xml(body)).unwrap().value;
        assert_eq!(
            score.provenance,
            if body.is_empty() {
                None
            } else {
                Some(Provenance {
                    encoding,
                    miscellaneous_empty: misc,
                })
            }
        );
        let restored = SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value;
        let output = MusicXml.write(&restored).unwrap().value;
        assert_eq!(MusicXml.read(&output).unwrap().value, score);
        assert_eq!(output.contains("<encoding>"), body.contains("encoding"));
        assert_eq!(output.contains("<miscellaneous/>"), misc);
    }
}
#[test]
fn claims_never_enable_unsupported_features_or_change_playback() {
    let score=MusicXml.read(&xml("<encoding><software>https://example.invalid/do-not-fetch</software><supports type=\"yes\" element=\"arpeggiate\"/><supports type=\"no\" element=\"note\"/></encoding>")).unwrap().value;
    let mut bare = score.clone();
    bare.provenance = None;
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&bare).unwrap().value).unwrap()
    );
    let unsupported = MusicXml
        .write(&score)
        .unwrap()
        .value
        .replace("</note>", "<notations><arpeggiate/></notations></note>");
    assert!(MusicXml.read(&unsupported).is_err());
}
#[test]
fn malformed_dates_claims_and_unknown_substantive_metadata_reject() {
    for value in [
        "2023-02-29",
        "1900-02-29",
        "2026-13-01",
        "2026-04-31",
        "0000-01-01",
        "2026-07-11Z",
        "2026-07-11+01:00",
        "2026-7-11",
        "date",
    ] {
        assert!(
            MusicXml
                .read(&xml(&format!(
                    "<encoding><encoding-date>{value}</encoding-date></encoding>"
                )))
                .is_err(),
            "{value}"
        );
    }
    for body in [
        "<encoding><encoder>Somebody</encoder></encoding>",
        "<encoding><encoding-description>text</encoding-description></encoding>",
        "<encoding><software unknown=\"x\">tool</software></encoding>",
        "<encoding><software><unknown/></software></encoding>",
        "<encoding><supports type=\"maybe\" element=\"note\"/></encoding>",
        "<encoding><supports type=\"yes\" element=\"note\" value=\"x\"/></encoding>",
        "<encoding><supports type=\"yes\" element=\"not a name\"/></encoding>",
        "<encoding><supports type=\"yes\"/></encoding>",
        "<encoding><supports type=\"yes\" element=\"note\">text</supports></encoding>",
        "<encoding><supports type=\"yes\" element=\"note\">\u{a0}</supports></encoding>",
        "<miscellaneous><miscellaneous-field name=\"future\">data</miscellaneous-field></miscellaneous>",
        "<miscellaneous>\u{a0}</miscellaneous>",
        "<encoding>text</encoding>",
        "<encoding/><encoding/>",
        "<miscellaneous/><miscellaneous/>",
    ] {
        assert!(MusicXml.read(&xml(body)).is_err(), "{body}");
    }
}
#[test]
fn exact_xml_dates_and_name_tokens_are_not_current_time_or_ascii_only() {
    for date in [
        "2000-02-29",
        "2400-02-29",
        "-0001-01-01",
        "10000-01-01",
        " 2026-07-11 ",
    ] {
        let source = xml(&format!(
            "<encoding><encoding-date>{date}</encoding-date><supports type=\" yes \" element=\" 音符 \" attribute=\"new-page\" value=\"a&#9;b\"/></encoding>"
        ));
        let score = MusicXml.read(&source).unwrap().value;
        let wire = SfsAdapter.write(&score).unwrap().value;
        assert_eq!(score, SfsAdapter.read(&wire).unwrap().value);
        assert_eq!(
            score,
            MusicXml
                .read(&MusicXml.write(&score).unwrap().value)
                .unwrap()
                .value
        );
    }
}
#[test]
fn public_model_and_native_wire_validate_closed_shape_and_scope() {
    let score = MusicXml.read(&xml(ENCODING)).unwrap().value;
    for invalid in [
        Provenance {
            encoding: None,
            miscellaneous_empty: false,
        },
        Provenance {
            encoding: Some(vec![EncodingEntry::Software {
                text: "\u{0}".into(),
            }]),
            miscellaneous_empty: false,
        },
        Provenance {
            encoding: Some(vec![EncodingEntry::Supports {
                supported: true,
                element: "note".into(),
                attribute: None,
                value: Some("".into()),
            }]),
            miscellaneous_empty: false,
        },
    ] {
        let mut bad = score.clone();
        bad.provenance = Some(invalid);
        assert!(MusicXml.write(&bad).is_err());
        assert!(SfsAdapter.write(&bad).is_err());
    }
    let doc = scorestream_codec::parse(&SfsAdapter.write(&score).unwrap().value).unwrap();
    for mode in 0..9 {
        let mut bad = doc.clone();
        if mode == 0 {
            bad.required_extensions.clear();
        }
        if mode == 1 {
            bad.document_kind = scorestream_core::DocumentKind::Fragment;
        }
        let mut duplicate = None;
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let Record::Mark(m) = r {
                    if m.kind == scorestream_core::provenance::HEADER {
                        match mode {
                            2 => m.target = "#n".into(),
                            3 => f.time = scorestream_core::Rational::one(),
                            4 => duplicate = Some(m.clone()),
                            5 => {
                                m.attributes
                                    .insert("encoding_present".into(), Scalar::Boolean(false));
                            }
                            _ => {}
                        }
                    }
                    if m.kind == scorestream_core::provenance::ENTRY {
                        match mode {
                            6 => {
                                m.attributes.insert("index".into(), Scalar::Integer(8));
                            }
                            7 => {
                                m.attributes.insert("extra".into(), Scalar::Boolean(true));
                            }
                            8 => m.kind = "scorestream.provenance.future".into(),
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some(m) = duplicate {
            bad.frames[0].records.push(Record::Mark(m));
        }
        assert!(
            scorestream_core::provenance::decode(&bad).is_err(),
            "mode {mode}"
        );
    }
}
