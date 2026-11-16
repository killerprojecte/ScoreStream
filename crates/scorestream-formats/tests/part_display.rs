use scorestream_formats::{MappingEffect, ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

fn two_flutes() -> String {
    let declarations = (1..=2).map(|i| format!("<score-part id=\"P{i}\"><part-name>Flute {i}</part-name><score-instrument id=\"I{i}\"><instrument-name>flute</instrument-name></score-instrument></score-part>")).collect::<String>();
    let parts = (1..=2).map(|i| format!("<part id=\"P{i}\"><measure number=\"1\"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note id=\"n{i}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure></part>")).collect::<String>();
    format!(
        "<score-partwise version=\"4.0\"><part-list>{declarations}</part-list>{parts}</score-partwise>"
    )
}

#[test]
fn distinct_flute_names_survive_musicxml_common_sfs_common_musicxml() {
    let original = MusicXml.read(&two_flutes()).unwrap().value;
    let text = SfsAdapter.write(&original).unwrap();
    assert!(
        text.value
            .contains("instrument=flute staves=1 display_name=\"Flute 1\"")
    );
    assert!(
        text.value
            .contains("instrument=flute staves=1 display_name=\"Flute 2\"")
    );
    assert!(
        !text
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "part_display_name")
    );
    let restored = SfsAdapter.read(&text.value).unwrap().value;
    assert_eq!(original, restored);
    let xml = MusicXml.write(&restored).unwrap().value;
    assert_eq!(MusicXml.read(&xml).unwrap().value, original);
    for part in restored.parts {
        assert_eq!(part.instrument.unwrap().0, "flute");
    }
}

#[test]
fn empty_and_escaped_labels_are_not_instrument_aliases() {
    for label in [
        "",
        "长笛 &amp; &lt;solo&gt; &quot;2&quot;",
        "a&#127;b",
        "a&#133;b",
        "a&#159;b",
        "a\u{85}b",
        "a\u{2028}b",
        "<![CDATA[a\u{85}\u{2028}b]]>",
    ] {
        let original = MusicXml
            .read(&two_flutes().replace("Flute 1", label))
            .unwrap()
            .value;
        let sfs = SfsAdapter.write(&original).unwrap().value;
        let restored = SfsAdapter.read(&sfs).unwrap().value;
        assert_eq!(original, restored);
        let xml = MusicXml.write(&restored).unwrap().value;
        assert!(!xml.contains("<solo>"));
        assert_eq!(MusicXml.read(&xml).unwrap().value, original);
    }
}

#[test]
fn xml10_label_text_preserves_nel_and_line_separator() {
    for label in [
        "a\u{85}\u{2028}b",
        "<![CDATA[a\u{85}\u{2028}b]]>",
        "a&#133;&#8232;b",
    ] {
        let score = MusicXml
            .read(&two_flutes().replace("Flute 1", label))
            .unwrap()
            .value;
        assert_eq!(score.parts[0].name.as_deref(), Some("a\u{85}\u{2028}b"));
    }
}

#[test]
fn recognized_idrefs_do_not_become_duplicate_definitions_or_allow_foreign_references() {
    let input = two_flutes().replace("</score-instrument>", "</score-instrument><midi-device id=\"I1\" port=\"1\"/><midi-instrument id=\"I1\"><midi-program>74</midi-program></midi-instrument>");
    let report = MusicXml.read(&input).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "instrument.idref" && d.effect == MappingEffect::Invalid)
    );
    assert!(!report.diagnostics.iter().any(|d| d.feature == "xml.id"));
    let input = two_flutes().replace(
        "<duration>1</duration>",
        "<duration>1</duration><instrument id=\"I1\"/>",
    );
    let report = MusicXml.read(&input).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "element.instrument")
    );
    assert!(!report.diagnostics.iter().any(|d| d.feature == "xml.id"));
    let duplicate = MusicXml
        .read(&two_flutes().replace("id=\"I2\"", "id=\"I1\""))
        .unwrap_err();
    assert!(
        duplicate
            .diagnostics
            .iter()
            .any(|d| d.feature == "xml.id" && d.effect == MappingEffect::Invalid)
    );
}

#[test]
fn richer_part_labels_and_layout_fail_closed_but_standard_sound_is_preserved() {
    for extra in [
        "<part-name-display><display-text>Flute 1</display-text></part-name-display>",
        "<part-abbreviation font-style=\"italic\">Fl. 1</part-abbreviation>",
    ] {
        assert!(
            MusicXml
                .read(&two_flutes().replace("</part-name>", &format!("</part-name>{extra}")))
                .is_err()
        );
    }
    assert!(
        MusicXml
            .read(&two_flutes().replace(
                "</instrument-name>",
                "</instrument-name><instrument-sound>wind.flutes.flute</instrument-sound>"
            ))
            .is_ok()
    );
    assert!(
        MusicXml
            .read(&two_flutes().replace("number=\"1\"", "number=\"1\" width=\"100\""))
            .is_err()
    );
}
