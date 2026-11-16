use scorestream_core::{
    Rational,
    lyrics::{EventLyrics, Lyric, Syllabic},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
const LYRIC: &str = r##"<lyric number="verse:二" color="#ff0000" default-x="0" default-y="-0.125" relative-x="1.25" relative-y="-30"><syllabic>single</syllabic><text>  A&amp;B&#9;&#10;&#13; II–V &lt;tag&gt;  </text></lyric>"##;
fn xml(lyric: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="P"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type>{lyric}</note></measure></part></score-partwise>"#
    )
}
#[test]
fn text_verse_order_style_and_absence_roundtrip_without_reinterpretation() {
    let second = "<lyric><syllabic>begin</syllabic><text/></lyric>";
    let score = MusicXml
        .read(&xml(&format!("{LYRIC}{second}")))
        .unwrap()
        .value;
    let g = &score.lyrics[0];
    assert_eq!(g.event, "n");
    assert_eq!(g.lyrics.len(), 2);
    assert_eq!(g.lyrics[0].number.as_deref(), Some("verse:二"));
    assert_eq!(g.lyrics[0].color.as_deref(), Some("#ff0000"));
    assert_eq!(g.lyrics[0].text, "  A&B\t\n\r II–V <tag>  ");
    assert_eq!(g.lyrics[0].default_x, Some(Rational::zero()));
    assert_eq!(g.lyrics[1].number, None);
    assert_eq!(g.lyrics[1].default_x, None);
    assert_eq!(g.lyrics[1].text, "");
    let wire = SfsAdapter.write(&score).unwrap().value;
    assert!(wire.contains("scorestream.lyrics@1"));
    let recovered = SfsAdapter.read(&wire).unwrap().value;
    assert_eq!(score.lyrics, recovered.lyrics);
    let output = MusicXml.write(&recovered).unwrap().value;
    let again = MusicXml.read(&output).unwrap().value;
    assert_eq!(score.lyrics, again.lyrics);
    let mut shuffled = scorestream_codec::parse(&wire).unwrap();
    for f in &mut shuffled.frames {
        f.records.reverse();
    }
    assert_eq!(
        scorestream_core::lyrics::decode(&shuffled).unwrap(),
        score.lyrics
    );
}
#[test]
fn unsupported_forms_are_rejected_not_stripped_or_flattened() {
    for lyric in [
        LYRIC.replace("</text>", "</text><extend/>"),
        LYRIC.replace("</text>", "</text><elision> </elision><text>more</text>"),
        LYRIC.replace("<text>", "<text font-style=\"italic\">"),
        LYRIC.replace("<text>", "<text><b/>"),
        LYRIC.replace("<syllabic>single</syllabic>", ""),
        LYRIC.replace("single", "invalid"),
        LYRIC.replace("#ff0000", "red"),
        LYRIC.replace("-0.125", "NaN"),
        LYRIC.replace("-0.125", "1/3"),
        LYRIC.replace("verse:二", "verse two"),
        LYRIC.replace("number=", "name="),
    ] {
        assert!(MusicXml.read(&xml(&lyric)).is_err(), "{lyric}");
    }
}
#[test]
fn native_ownership_onset_indices_closed_fields_and_versions_are_checked() {
    let score = MusicXml.read(&xml(LYRIC)).unwrap().value;
    let wire = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&wire).unwrap();
    let mut bad = doc.clone();
    bad.required_extensions.clear();
    assert!(
        scorestream_core::validate(&bad, None)
            .iter()
            .any(|d| d.code == "lyrics.wire")
    );
    for (key, v) in [
        ("index", scorestream_core::Scalar::Integer(1)),
        ("index", scorestream_core::Scalar::Integer(-1)),
        ("extra", scorestream_core::Scalar::Boolean(true)),
        ("color", scorestream_core::Scalar::String("red".into())),
        ("default_y", scorestream_core::Scalar::String("1/3".into())),
    ] {
        let mut bad = doc.clone();
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let scorestream_core::Record::Mark(m) = r {
                    if m.kind == scorestream_core::lyrics::KIND {
                        m.attributes.insert(key.into(), v.clone());
                    }
                }
            }
        }
        assert!(scorestream_core::lyrics::decode(&bad).is_err());
    }
    let mut bad = doc.clone();
    for f in &mut bad.frames {
        let copies:Vec<_>=f.records.iter().filter(|r|matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::lyrics::KIND)).cloned().collect();
        f.records.extend(copies);
    }
    assert!(scorestream_core::lyrics::decode(&bad).is_err());
    let mut bad = doc.clone();
    let mut marks = vec![];
    for f in &mut bad.frames {
        f.records.retain(|r|if matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::lyrics::KIND){marks.push(r.clone());false}else{true});
    }
    let mut late = bad.frames[0].clone();
    late.time = Rational::from_integer(1);
    late.records = marks;
    bad.frames.push(late);
    assert!(scorestream_core::lyrics::decode(&bad).is_err());
    let mut bad = doc.clone();
    bad.required_extensions = vec!["scorestream.lyrics@2".into()];
    assert!(
        scorestream_core::validate(&bad, None)
            .iter()
            .any(|d| d.code == "extension_unsupported")
    );
}
#[test]
fn public_model_never_exports_invalid_lyric_and_audio_is_identical() {
    let high = format!("0.{}1", "0".repeat(90));
    let score = MusicXml
        .read(&xml(&LYRIC.replace("-0.125", &high)))
        .unwrap()
        .value;
    let mut plain = score.clone();
    plain.lyrics.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    assert!(MusicXml.write(&score).unwrap().value.contains(&high));
    for g in [
        EventLyrics {
            event: "missing".into(),
            ..score.lyrics[0].clone()
        },
        EventLyrics {
            event: "n".into(),
            lyrics: vec![],
        },
        EventLyrics {
            event: "n".into(),
            lyrics: vec![Lyric {
                text: "bad\0".into(),
                ..score.lyrics[0].lyrics[0].clone()
            }],
        },
    ] {
        let mut bad = score.clone();
        bad.lyrics = vec![g];
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
    let mut duplicate = score.clone();
    duplicate.lyrics.push(score.lyrics[0].clone());
    assert!(SfsAdapter.write(&duplicate).is_err());
    assert_eq!(score.lyrics[0].lyrics[0].syllabic, Syllabic::Single);
}
#[test]
fn chord_and_grace_lyrics_retain_their_own_event_not_the_principal() {
    let chord = format!(
        r#"<note id="ch"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>whole</type>{LYRIC}</note>"#
    );
    let input = xml("").replace("</measure>", &format!("{chord}</measure>"));
    let score = MusicXml.read(&input).unwrap().value;
    assert_eq!(score.lyrics[0].event, "ch");
    let recovered = SfsAdapter
        .read(&SfsAdapter.write(&score).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(score.lyrics, recovered.lyrics);
    let grace = format!(
        r#"<note id="g"><grace/><pitch><step>D</step><octave>4</octave></pitch><type>eighth</type>{LYRIC}</note>"#
    );
    let input = xml("").replace("<note id=\"n\">", &format!("{grace}<note id=\"n\">"));
    let score = MusicXml.read(&input).unwrap().value;
    assert_eq!(score.lyrics[0].event, "g");
    let recovered = SfsAdapter
        .read(&SfsAdapter.write(&score).unwrap().value)
        .unwrap()
        .value;
    let again = MusicXml
        .read(&MusicXml.write(&recovered).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(score.lyrics, again.lyrics);
}
