use scorestream_core::{Rational, defaults::*};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn xml(defaults: &str) -> String {
    format!(
        r#"<score-partwise version="4.0">{defaults}<part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id="e"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#
    )
}
fn roundtrip(defaults: &str) -> scorestream_core::score::ScoreDocument {
    let imported = MusicXml.read(&xml(defaults)).unwrap();
    let score = imported.value;
    if score.defaults.is_some() {
        assert!(
            imported
                .report
                .diagnostics
                .iter()
                .any(|d| d.feature == "defaults.authored")
        );
    }
    let native = SfsAdapter.write(&score).unwrap().value;
    let again = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(score.defaults, again.defaults);
    let output = MusicXml.write(&again).unwrap().value;
    let recovered = MusicXml.read(&output).unwrap().value;
    assert_eq!(score.defaults, recovered.defaults);
    assert_eq!(score.parts, recovered.parts);
    let mut plain = score.clone();
    plain.defaults = None;
    assert_eq!(
        scorestream_app::playback::plan_source(&native).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    score
}
#[test]
fn absent_empty_sections_and_optional_font_fields_are_distinct() {
    assert!(roundtrip("").defaults.is_none());
    let d = roundtrip("<defaults/>").defaults.unwrap();
    assert_eq!(d, Defaults::default());
    let d=roundtrip("<defaults><page-layout/><appearance/><music-font/><word-font font-family=\"\"/><lyric-font/><lyric-font font-size=\"10\"/></defaults>").defaults.unwrap();
    assert_eq!(d.page_layout, Some(PageLayout::default()));
    assert_eq!(d.appearance, Some(vec![]));
    assert_eq!(d.music_font, Some(Font::default()));
    assert_eq!(d.word_font.unwrap().family, Some(String::new()));
    assert_eq!(d.lyric_fonts.len(), 2);
    assert_eq!(d.lyric_fonts[0], Font::default());
}
#[test]
fn exact_numbers_order_group_types_and_escaped_fonts() {
    let source = r#"<defaults><scaling><millimeters>4.6736</millimeters><tenths>40</tenths></scaling><page-layout><page-height>12345678901234567890123456789.12345</page-height><page-width>0</page-width><page-margins type="even"><left-margin>-0.125</left-margin><right-margin>0</right-margin><top-margin>115.385</top-margin><bottom-margin>2</bottom-margin></page-margins><page-margins type="odd"><left-margin>1</left-margin><right-margin>2</right-margin><top-margin>3</top-margin><bottom-margin>4</bottom-margin></page-margins></page-layout><appearance><line-width type="slur tip">0.7</line-width><line-width type="slur tip">0</line-width><note-size type="grace-cue">49</note-size><note-size type="cue">0</note-size></appearance><music-font font-family="A &amp; B &quot;C&quot; &#9;&#10;&#13;"/><word-font font-size="10.25"/><lyric-font font-family="FreeSerif" font-size="30"/></defaults>"#;
    let d = roundtrip(source).defaults.unwrap();
    let p = d.page_layout.unwrap();
    assert_eq!(p.margins[0].margin_type, Some(MarginType::Even));
    assert_eq!(p.margins[0].left, "-1/8".parse::<Rational>().unwrap());
    assert_eq!(p.margins[1].margin_type, Some(MarginType::Odd));
    assert_eq!(d.appearance.unwrap().len(), 4);
    assert_eq!(
        d.music_font.unwrap().family.as_deref(),
        Some("A & B \"C\" \t\n\r")
    );
    for t in ["", " type=\"both\""] {
        let d=roundtrip(&format!("<defaults><page-layout><page-margins{t}><left-margin>0</left-margin><right-margin>0</right-margin><top-margin>0</top-margin><bottom-margin>0</bottom-margin></page-margins></page-layout></defaults>")).defaults.unwrap();
        assert_eq!(
            d.page_layout.unwrap().margins[0].margin_type,
            if t.is_empty() {
                None
            } else {
                Some(MarginType::Both)
            }
        );
    }
}
#[test]
fn unknown_defaults_or_invalid_structures_do_not_disappear() {
    for d in [
        "<defaults><concert-score/></defaults>",
        "<defaults><system-layout/></defaults>",
        "<defaults><staff-layout/></defaults>",
        "<defaults><lyric-language xml:lang=\"en\"/></defaults>",
        "<defaults><scaling><millimeters>0</millimeters><tenths>40</tenths></scaling></defaults>",
        "<defaults><scaling><millimeters>1/3</millimeters><tenths>40</tenths></scaling></defaults>",
        "<defaults><page-layout><page-height>1</page-height></page-layout></defaults>",
        "<defaults><appearance><line-width type=\"unknown\">1</line-width></appearance></defaults>",
        "<defaults><appearance><distance type=\"hyphen\">1</distance></appearance></defaults>",
        "<defaults><appearance><note-size type=\"cue\">-1</note-size></appearance></defaults>",
        "<defaults><appearance><note-size type=\"cue\">70</note-size><line-width type=\"beam\">1</line-width></appearance></defaults>",
        "<defaults><word-font font-size=\"large\"/></defaults>",
        "<defaults><music-font font-weight=\"bold\"/></defaults>",
        "<defaults><lyric-font number=\"1\" font-family=\"F\"/></defaults>",
        "<defaults><word-font/><music-font/></defaults>",
        "<defaults/><defaults/>",
        "<defaults><appearance/><appearance/></defaults>",
    ] {
        assert!(MusicXml.read(&xml(d)).is_err(), "{d}");
    }
}
#[test]
fn native_capability_parenting_fields_indices_and_timing_are_closed() {
    let score = roundtrip(
        "<defaults><scaling><millimeters>5.2</millimeters><tenths>40</tenths></scaling><appearance><line-width type=\"beam\">5</line-width></appearance></defaults>",
    );
    let native = SfsAdapter.write(&score).unwrap().value;
    for bad in [
        native.replace("scorestream.defaults@1", "scorestream.unknown@1"),
        native.replace("index=0", "index=7"),
        native.replace("index=1", "index=0"),
        native.replace("kind=root", "kind=unknown"),
        native.replace("kind=scaling", "kind=scaling foo=true"),
        native.replace("millimeters=26/5", "millimeters=1/3"),
        native.replace("+ score scorestream.defaults", "+ #e scorestream.defaults"),
    ] {
        assert_ne!(
            bad, native,
            "mutation did not match native source:\n{native}"
        );
        assert!(SfsAdapter.read(&bad).is_err(), "{bad}");
    }
    let mut model = score.clone();
    model
        .defaults
        .as_mut()
        .unwrap()
        .scaling
        .as_mut()
        .unwrap()
        .tenths = Rational::zero();
    assert!(SfsAdapter.write(&model).is_err());
    assert!(MusicXml.write(&model).is_err());
}

#[test]
fn defaults_are_score_wide_not_misplaced_print_or_late_headers() {
    let base = xml("");
    for bad in [
        base.replace("</part-list>", "</part-list><defaults/>"),
        base.replace(
            "</part></score-partwise>",
            "</part><defaults/></score-partwise>",
        ),
        base.replace(
            "<part-list>",
            "<credit><credit-words>Title</credit-words></credit><defaults/><part-list>",
        ),
        base.replace("<attributes>", "<print><page-layout/></print><attributes>"),
    ] {
        assert!(MusicXml.read(&bad).is_err());
    }
}
