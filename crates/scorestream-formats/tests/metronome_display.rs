use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>2</divisions></attributes>{body}<note id="metronome_display_1"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note></measure></part></score-partwise>"#
    )
}
fn group(p: &str, tail: &str, sound: &str) -> String {
    format!(
        r#"<direction placement="above" system="only-top"><direction-type><words default-x="-32.02" relative-y="20" font-size="12" font-weight="bold">Presto [</words></direction-type><direction-type><metronome{p}><beat-unit>half</beat-unit><per-minute>112</per-minute></metronome></direction-type>{tail}{sound}</direction>"#
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
    for m in &mut plain.metronomes {
        m.presentation = None;
    }
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
fn ordered_group_geometry_parentheses_and_one_numeric_host_roundtrip_without_clock_inference() {
    let tail = r#"<direction-type><words font-family="Edwin" font-style="italic">] &#10; </words></direction-type>"#;
    let (m, s) = roundtrip(&score(&group(
        r#" parentheses="no" default-x="-32.02" default-y="0" relative-x="-1.25" relative-y="20""#,
        tail,
        r#"<sound tempo="71"/>"#,
    )));
    assert_eq!(m.metronomes.len(), 1);
    assert!(m.words.is_empty());
    assert_eq!(m.metronomes[0].value.bpm.to_string(), "112");
    assert_eq!(m.tempos[0].quarter_bpm.to_string(), "71");
    let p = m.metronomes[0].presentation.as_ref().unwrap();
    assert_eq!(p.parentheses, Some(false));
    assert_eq!(p.default_y, Some("0".parse().unwrap()));
    assert_eq!(p.relative_x, Some("-5/4".parse().unwrap()));
    assert_eq!(
        p.trailing_words.as_ref().unwrap().text,
        "] \n "
    );
    assert_eq!(p.leading_words.as_ref().unwrap().text, "Presto [");
    let doc = scorestream_formats::sfs::to_document(&m).unwrap().value;
    let host = doc
        .frames
        .iter()
        .flat_map(|f| &f.records)
        .filter_map(|r| match r {
            scorestream_core::Record::Mark(m) if m.kind == "metronome" => Some(m),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(host.len(), 1);
    assert_ne!(
        host[0].attributes["id"].as_str(),
        Some("metronome_display_1")
    );
    unchanged(&m, &s);
    let view = scorestream_app::Workspace::from_source(&s)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "metronome_display.authored_only")
    );
}
#[test]
fn absent_yes_no_parentheses_and_plain_numeric_marks_are_distinct() {
    for attr in ["", r#" parentheses="yes""#, r#" parentheses="no""#] {
        let (m, s) = roundtrip(&score(&group(attr, "", "")));
        assert_eq!(
            m.metronomes[0].presentation.as_ref().unwrap().parentheses,
            match attr {
                "" => None,
                _ => Some(attr.contains("yes")),
            }
        );
        assert!(m.tempos.is_empty());
        unchanged(&m, &s);
    }
    let plain = r#"<direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>60</per-minute></metronome></direction-type></direction>"#;
    let (m, _) = roundtrip(&score(plain));
    assert!(m.metronomes[0].presentation.is_none());
    let (m, s) = roundtrip(&score(&plain.replace(
        "<metronome>",
        "<metronome parentheses=\"yes\" relative-x=\"0\">",
    )));
    assert_eq!(
        m.metronomes[0].presentation.as_ref().unwrap().relative_x,
        Some("0".parse().unwrap())
    );
    unchanged(&m, &s);
}
#[test]
fn visual_signed_offsets_keep_numeric_sound_clock_separate() {
    for flag in ["yes", "no"] {
        let note = r#"<note id="first"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>"#;
        let g = group("", "", "").replace(
            "</direction>",
            &format!("<offset sound=\"{flag}\">-2</offset><sound tempo=\"71\"/></direction>"),
        );
        let (m, s) = roundtrip(
            &score(&format!("{note}{g}"))
                .replace("<duration>8</duration>", "<duration>6</duration>"),
        );
        assert_eq!(m.metronomes[0].at.to_string(), "0");
        assert_eq!(
            m.tempos[0].at.to_string(),
            if flag == "yes" { "0" } else { "1" }
        );
        unchanged(&m, &s);
    }
}
#[test]
fn unrelated_composites_and_unrepresented_shapes_fail_closed() {
    let g = group("", "", "");
    for bad in [
        g.replace("<metronome>", "<metronome parentheses=\"maybe\">"),
        g.replace("<metronome>", "<metronome font-size=\"12\">"),
        g.replace("<words ", "<words color=\"#FF0000\" "),
        g.replace(
            "<beat-unit>half</beat-unit>",
            "<beat-unit>half</beat-unit><beat-unit-dot/><beat-unit-dot/>",
        ),
        g.replace(
            "</direction>",
            "<direction-type><dynamics><p/></dynamics></direction-type></direction>",
        ),
        g.replace(
            "<per-minute>112</per-minute>",
            "<per-minute>Allegro</per-minute>",
        ),
    ] {
        assert!(MusicXml.read(&score(&bad)).is_err(), "{bad}");
    }
}
#[test]
fn metadata_requires_exact_unique_existing_numeric_host_owner_and_closed_words() {
    let (m, _) = roundtrip(&score(&group(r#" parentheses="yes""#, "", "")));
    for mode in 0..8 {
        let mut d = scorestream_formats::sfs::to_document(&m).unwrap().value;
        if mode == 0 {
            d.required_extensions
                .retain(|s| s != scorestream_core::metronome_display::EXTENSION);
        }
        let frame=d.frames.iter_mut().find(|f|f.records.iter().any(|r|matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::metronome_display::KIND))).unwrap();
        let mark = frame
            .records
            .iter_mut()
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m)
                    if m.kind == scorestream_core::metronome_display::KIND =>
                {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        use scorestream_core::Scalar;
        match mode {
            1 => {
                mark.target = "#missing".into();
            }
            2 => {
                mark.attributes
                    .insert("measure".into(), Scalar::String("other".into()));
            }
            3 => {
                mark.attributes.remove("leading_text");
            }
            4 => {
                mark.attributes
                    .insert("parentheses".into(), Scalar::String("no".into()));
            }
            5 => {
                mark.attributes
                    .insert("default_x".into(), Scalar::String("1/3".into()));
            }
            6 => {
                mark.attributes
                    .insert("bpm".into(), Scalar::String("1".into()));
            }
            7 => {
                let copy = mark.clone();
                frame.records.push(scorestream_core::Record::Mark(copy));
            }
            _ => {}
        }
        assert!(scorestream_formats::sfs::from_document(&d).is_err());
    }
    let mut bad = m;
    bad.metronomes[0]
        .presentation
        .as_mut()
        .unwrap()
        .leading_words
        .as_mut()
        .unwrap()
        .text = "".into();
    assert!(MusicXml.write(&bad).is_err());
}

#[test]
fn equal_numeric_marks_keep_distinct_ordered_presentations() {
    let a = group(r#" parentheses="yes""#, "", "");
    let b = a.replace("Presto [", "Second");
    let (m, s) = roundtrip(&score(&format!("{a}{b}")));
    assert_eq!(m.metronomes.len(), 2);
    assert_eq!(
        m.metronomes[0]
            .presentation
            .as_ref()
            .unwrap()
            .leading_words
            .as_ref()
            .unwrap()
            .text,
        "Presto ["
    );
    assert_eq!(
        m.metronomes[1]
            .presentation
            .as_ref()
            .unwrap()
            .leading_words
            .as_ref()
            .unwrap()
            .text,
        "Second"
    );
    unchanged(&m, &s);
}
