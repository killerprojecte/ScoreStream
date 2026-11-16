use scorestream_core::score::TimeSymbol;
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn xml(time: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions>{time}<clef><sign>G</sign><line>2</line></clef></attributes><note id="e"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#
    )
}
#[test]
fn common_cut_absent_are_exact_and_passive() {
    for (symbol, n, d) in [(Some("common"), 4, 4), (Some("cut"), 2, 2), (None, 4, 4)] {
        let time = format!(
            "<time{}><beats>{n}</beats><beat-type>{d}</beat-type></time>",
            symbol
                .map(|s| format!(" symbol=\"{s}\""))
                .unwrap_or_default()
        );
        let score = MusicXml.read(&xml(&time)).unwrap().value;
        assert_eq!(
            score.parts[0].measures[0].states[0]
                .meter
                .as_ref()
                .unwrap()
                .symbol,
            symbol.and_then(TimeSymbol::from_name)
        );
        let native = SfsAdapter.write(&score).unwrap().value;
        let again = SfsAdapter.read(&native).unwrap().value;
        assert_eq!(score.parts, again.parts);
        let written = MusicXml.write(&again).unwrap().value;
        assert_eq!(score.parts, MusicXml.read(&written).unwrap().value.parts);
        let mut plain = score.clone();
        plain.parts[0].measures[0].states[0]
            .meter
            .as_mut()
            .unwrap()
            .symbol = None;
        assert_eq!(
            scorestream_app::playback::plan_source(&native).unwrap(),
            scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                .unwrap()
        );
    }
}
#[test]
fn staff_ownership_and_new_meter_without_symbol_reset() {
    let input = xml(
        "<staves>2</staves><time number=\"1\" symbol=\"common\"><beats>4</beats><beat-type>4</beat-type></time><time number=\"2\" symbol=\"cut\"><beats>2</beats><beat-type>2</beat-type></time>",
    );
    let input=input.replace("</part></score-partwise>","<measure id=\"m2\" number=\"2\"><attributes><time number=\"1\"><beats>4</beats><beat-type>4</beat-type></time></attributes><note id=\"e2\"><rest/><duration>4</duration><type>whole</type></note></measure></part></score-partwise>");
    let score = MusicXml.read(&input).unwrap().value;
    let a = &score.parts[0].measures;
    assert_eq!(
        a[0].states[0].meter.as_ref().unwrap().symbol,
        Some(TimeSymbol::Common)
    );
    assert_eq!(
        a[0].states[1].meter.as_ref().unwrap().symbol,
        Some(TimeSymbol::Cut)
    );
    assert_eq!(a[1].states[0].meter.as_ref().unwrap().symbol, None);
    let native = SfsAdapter.write(&score).unwrap().value;
    assert_eq!(score.parts, SfsAdapter.read(&native).unwrap().value.parts);
    assert_eq!(
        score.parts,
        MusicXml
            .read(&MusicXml.write(&score).unwrap().value)
            .unwrap()
            .value
            .parts
    );
}
#[test]
fn mismatches_unknown_symbols_and_native_orphans_reject() {
    for (symbol, n, d) in [
        ("normal", 4, 4),
        ("single-number", 4, 4),
        ("common", 3, 4),
        ("cut", 4, 4),
        ("cut", 2, 4),
        ("none", 4, 4),
    ] {
        assert!(
            MusicXml
                .read(&xml(&format!(
                    "<time symbol=\"{symbol}\"><beats>{n}</beats><beat-type>{d}</beat-type></time>"
                )))
                .is_err()
        );
    }
    let score = MusicXml
        .read(&xml(
            "<time symbol=\"common\"><beats>4</beats><beat-type>4</beat-type></time>",
        ))
        .unwrap()
        .value;
    let native = SfsAdapter.write(&score).unwrap().value;
    for bad in [
        native.replace("meter_symbol=common", "meter_symbol=cut"),
        native.replace("meter=4/4 ", ""),
        native.replace("meter_symbol=common", "meter_symbol=normal"),
        native.replace("= p.s1", "= p"),
        native.replace(
            "meter_symbol=common",
            "meter_symbol=common\n= p.s1 meter=4/4",
        ),
    ] {
        assert_ne!(bad, native);
        assert!(SfsAdapter.read(&bad).is_err(), "{bad}");
    }
    let mut invalid = score;
    invalid.parts[0].measures[0].states[0]
        .meter
        .as_mut()
        .unwrap()
        .symbol = Some(TimeSymbol::Cut);
    assert!(SfsAdapter.write(&invalid).is_err());
    assert!(MusicXml.write(&invalid).is_err());
}
#[test]
fn display_does_not_change_numeric_irregular_extent_contract() {
    use scorestream_core::score::Meter;
    use scorestream_formats::musicxml::{self, IrregularMeasure, MusicXmlOptions};
    let options = MusicXmlOptions {
        irregular_measures: vec![IrregularMeasure {
            ordinal: 1,
            label: "1".into(),
            written_meter: Meter {
                symbol: None,
                beats: vec![4],
                beat_type: 4,
            },
            actual_length: "5".parse().unwrap(),
        }],
        ..Default::default()
    };
    let input = xml("<time symbol=\"common\"><beats>4</beats><beat-type>4</beat-type></time>")
        .replace("<duration>4</duration>", "<duration>5</duration>");
    let score = musicxml::read_with_options(&input, &options).unwrap().value;
    assert_eq!(score.parts[0].measures[0].length.to_string(), "5");
    assert_eq!(
        score.parts[0].measures[0].states[0]
            .meter
            .as_ref()
            .unwrap()
            .symbol,
        Some(TimeSymbol::Common)
    );
}
