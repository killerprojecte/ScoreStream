use scorestream_core::{Pitch, Rational};
use scorestream_formats::{MappingEffect, ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
const TWO: &str = include_str!("fixtures/two-staff.musicxml");
fn normalized(mut score: scorestream_core::score::ScoreDocument) -> serde_json::Value {
    score.metadata.source.clear();
    for part in &mut score.parts {
        for measure in &mut part.measures {
            for e in &mut measure.events {
                if e.chord.is_some() {
                    e.chord = Some("chord".into());
                }
            }
        }
    }
    serde_json::to_value(score).unwrap()
}
#[test]
fn two_staff_divisions_backup_forward_and_chord_durations() {
    let imported = MusicXml.read(TWO).unwrap();
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .all(|d| d.effect == MappingEffect::Normalized)
    );
    let p = &imported.value.parts[0];
    assert_eq!(p.staves, 2);
    assert_eq!(p.voices.len(), 2);
    let m = &p.measures[0];
    assert_eq!(m.events[0].offset, Rational::zero());
    assert_eq!(m.events[0].duration, "2".parse().unwrap());
    assert_eq!(m.events[1].duration, Rational::one());
    assert_eq!(m.events[0].chord, m.events[1].chord);
    assert_eq!(m.events[2].offset, "3".parse().unwrap());
    assert_eq!(m.events[2].pitch, Pitch::Rest);
    assert_eq!(m.events[3].offset, Rational::zero());
    assert_eq!(m.events[3].staff, 2);
    assert_eq!(p.measures[1].start, "4".parse().unwrap());
    assert_eq!(p.measures[1].length, "3".parse().unwrap());
    assert_eq!(p.measures[1].events[0].duration, "1/3".parse().unwrap());
    assert_eq!(
        imported.value.metadata.title.as_deref(),
        Some("Two staves & exact durations")
    );
}
#[test]
fn musicxml_model_roundtrip_preserves_supported_semantics() {
    let score = MusicXml.read(TWO).unwrap().value;
    let written = MusicXml.write(&score).unwrap().value;
    let again = MusicXml.read(&written).unwrap().value;
    assert_eq!(normalized(score), normalized(again));
    assert!(written.contains("<backup>"));
    assert!(written.contains("<chord/>"));
}
#[test]
fn xml_sfs_xml_real_roundtrip() {
    let score = MusicXml.read(TWO).unwrap().value;
    let sfs = SfsAdapter.write(&score).unwrap().value;
    let from_sfs = SfsAdapter.read(&sfs).unwrap().value;
    let xml = MusicXml.write(&from_sfs).unwrap().value;
    let again = MusicXml.read(&xml).unwrap().value;
    assert_eq!(normalized(score), normalized(again));
}
#[test]
fn unsupported_paths_are_reported_and_no_document_returned() {
    let report = MusicXml
        .read(include_str!("fixtures/unsupported.musicxml"))
        .unwrap_err();
    assert!(
        report.diagnostics.iter().any(
            |d| d.feature == "element.direction" && d.path.contains("/measure[1]/direction[1]")
        )
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "element.notations" && d.source.unwrap().line > 0)
    );
    for input in [
        TWO.replace(
            "<note id=\"head\">",
            "<note id=\"head\" print-object=\"no\">",
        ),
        TWO.replace(
            "<fifths>-1</fifths>",
            "<fifths>-1</fifths><mode>minor</mode>",
        ),
        TWO.replace(
            "<clef number=\"1\">",
            "<clef number=\"1\" additional=\"yes\">",
        ),
    ] {
        assert!(MusicXml.read(&input).is_err());
    }
}
#[test]
fn no_external_entities_dtd_or_processing_instructions() {
    for prefix in [
        "<!DOCTYPE score-partwise SYSTEM 'file:///etc/passwd'>",
        "<!DOCTYPE score-partwise [<!ENTITY x SYSTEM 'https://example.invalid/secret'>]>",
        "<?custom href='x'?>",
    ] {
        let input = format!(
            "{prefix}{}",
            TWO.lines().skip(1).collect::<Vec<_>>().join("\n")
        );
        assert!(MusicXml.read(&input).is_err());
    }
    assert!(
        MusicXml
            .read(&TWO.replace("Two staves &amp; exact durations", "&external;"))
            .is_err()
    );
    assert!(
        MusicXml
            .read(&TWO.replace("Two staves &amp; exact durations", "&#0;"))
            .is_err()
    );
}
#[test]
fn bounded_xml_tokens_and_depth() {
    // Limits apply to an individual pathological token, not the whole score.
    assert!(
        MusicXml
            .read(&format!("<x>{}</x>", "x".repeat(1024 * 1024 + 1)))
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.effect == MappingEffect::ResourceLimit)
    );
    let nested = format!("{}{}", "<x>".repeat(50), "</x>".repeat(50));
    assert!(
        MusicXml
            .read(&nested)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.effect == MappingEffect::ResourceLimit)
    );
}
#[test]
fn malformed_timing_and_cardinality_are_rejected() {
    for input in [
        TWO.replace(
            "<backup><duration>24</duration>",
            "<backup><duration>25</duration>",
        ),
        TWO.replacen("<duration>6</duration>", "<duration>13</duration>", 1),
        TWO.replacen("<duration>12</duration>", "<duration>0</duration>", 1),
        TWO.replace("<divisions>6</divisions>", "<divisions>0</divisions>"),
        TWO.replace(
            "<divisions>6</divisions>",
            "<divisions>6</divisions><divisions>7</divisions>",
        ),
        TWO.replace("<staff>2</staff>", "<staff>3</staff>"),
        TWO.replace("<octave>4</octave>", "<octave>10</octave>"),
        TWO.replace("<note id=\"head\">", "<note id=\"head\"><chord/>"),
        TWO.replace("</score-partwise>", ""),
        TWO.replace(
            "</score-partwise>",
            "</score-partwise><score-partwise version=\"4.0\"/>",
        ),
    ] {
        assert!(MusicXml.read(&input).is_err(), "{input}");
    }
}
#[test]
fn exact_decimal_divisions_do_not_use_floats() {
    let xml = TWO
        .replace("<divisions>6</divisions>", "<divisions>+06.00</divisions>")
        .replace("<duration>12</duration>", "<duration>12.000</duration>");
    assert_eq!(
        normalized(MusicXml.read(TWO).unwrap().value),
        normalized(MusicXml.read(&xml).unwrap().value)
    );
}
#[test]
fn shared_metadata_is_escaped_or_rejected_without_loss() {
    let mut score = MusicXml.read(TWO).unwrap().value;
    score.metadata.title = Some("<Script> & \"title\"\r\n次".into());
    score.metadata.authors = vec!["林".into(), "Alex & Jo".into()];
    score.metadata.composer = Some("Composer".into());
    score.metadata.lyricist = Some("Words".into());
    score.metadata.arranger = Some("Arrange".into());
    score.metadata.work_number = Some("Op. 1".into());
    score.metadata.movement = Some("I".into());
    score.metadata.rights = Some("© 2026".into());
    let xml = MusicXml.write(&score).unwrap().value;
    assert!(!xml.contains("<Script>"));
    assert_eq!(MusicXml.read(&xml).unwrap().value.metadata, score.metadata);
    score.metadata.header = Some("Private".into());
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "metadata.header")
    );
    score.metadata.header = None;
    score.metadata.title = Some("bad\u{1}".into());
    assert!(MusicXml.write(&score).is_err());
}
#[test]
fn unsupported_export_range_overlap_and_chord_heads_are_rejected() {
    let base = MusicXml.read(TWO).unwrap().value;
    let mut score = base.clone();
    score.parts[0].measures[0].events[0].pitch = "C(+1/3)4".parse().unwrap();
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "pitch.alter")
    );
    let mut score = base.clone();
    score.parts[0].measures[0].events[1].duration = "3".parse().unwrap();
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "chord.duration")
    );
    let mut score = base;
    score.parts[0].measures[0].events[1].chord = None;
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "voice.overlap")
    );
}
#[test]
fn implicit_pickup_and_inherited_meter() {
    let input = TWO.replacen(
        "<measure number=\"1\">",
        "<measure number=\"0\" implicit=\"yes\">",
        1,
    );
    let score = MusicXml.read(&input).unwrap().value;
    assert!(score.parts[0].measures[0].implicit);
    assert!(
        MusicXml
            .read(&MusicXml.write(&score).unwrap().value)
            .is_ok()
    );
}

#[test]
fn generated_ids_never_collide_with_source_ids() {
    let xml = TWO
        .replace("id=\"head\"", "id=\"m1_1\"")
        .replace("id=\"short\"", "id=\"mx_n3\"")
        .replace("id=\"rest\"", "");
    let score = MusicXml.read(&xml).unwrap().value;
    assert_ne!(score.parts[0].measures[0].id, "m1_1");
    assert!(MusicXml.write(&score).is_ok());
}

#[test]
fn real_pickup_and_empty_full_measure_roundtrip() {
    let source = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name/></score-part></part-list><part id="p"><measure number="0" implicit="yes"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><rest/><duration>1</duration></note></measure><measure number="1"/></part></score-partwise>"#;
    let score = MusicXml.read(source).unwrap().value;
    assert_eq!(score.parts[0].measures[0].length, Rational::one());
    assert_eq!(score.parts[0].measures[1].start, Rational::one());
    assert_eq!(score.parts[0].measures[1].length, "4".parse().unwrap());
    assert!(score.parts[0].measures[1].events.is_empty());
    assert_eq!(
        normalized(score.clone()),
        normalized(
            MusicXml
                .read(&MusicXml.write(&score).unwrap().value)
                .unwrap()
                .value
        )
    );
}
#[test]
fn long_chord_head_controls_all_tails_not_previous_tail() {
    let tail = r#"<note id="third"><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>9</duration><voice>1</voice><staff>1</staff></note>"#;
    let source = TWO.replacen(
        "<forward><duration>6",
        &format!("{tail}<forward><duration>6"),
        1,
    );
    let score = MusicXml.read(&source).unwrap().value;
    assert_eq!(
        score.parts[0].measures[0].events[2].duration,
        "3/2".parse().unwrap()
    );
    assert!(
        MusicXml
            .read(&MusicXml.write(&score).unwrap().value)
            .is_ok()
    );
}
#[test]
fn mid_measure_attributes_are_not_applied_in_document_order() {
    let source = TWO.replace(
        "<forward><duration>6",
        "<attributes><key><fifths>2</fifths></key></attributes><forward><duration>6",
    );
    let report = MusicXml.read(&source).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "attributes.mid_measure")
    );
}
#[test]
fn exported_public_models_cannot_contain_invalid_xml_or_unknown_fields() {
    let mut score = MusicXml.read(TWO).unwrap().value;
    let old = score.parts[0].voices[0].id.clone();
    score.parts[0].voices[0].id.0 = "bad\u{1}".into();
    let new = score.parts[0].voices[0].id.clone();
    for m in &mut score.parts[0].measures {
        for e in &mut m.events {
            if e.voice == old {
                e.voice = new.clone();
            }
        }
    }
    assert!(MusicXml.write(&score).is_err());
    let mut json = serde_json::to_value(&score).unwrap();
    json["passthrough_markup"] = "<foreign/>".into();
    assert!(serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err());
}
