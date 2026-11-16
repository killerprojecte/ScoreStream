//! Independent acceptance for automatic per-note retention, not desktop readiness.
use scorestream_core::score::ScoreDocument;
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};

const STUDY: &str = include_str!("fixtures/articulation-study.musicxml");
fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        articulations: ArticulationProfile::Automatic,
        ..Default::default()
    }
}
fn read(xml: &str) -> ScoreDocument {
    musicxml::read_with_options(xml, &options()).unwrap().value
}
fn native(score: &ScoreDocument) -> String {
    SfsAdapter.write(score).unwrap().value
}
fn compatible(xml: &str) {
    let old = MusicXml.read(xml).unwrap().value;
    let new = read(xml);
    assert_eq!(new, old);
    assert!(new.authored_articulations.is_none());
    assert!(new.notation_order.is_none());
    assert_eq!(native(&new), native(&old));
    let old_plan = scorestream_app::playback::plan_source(&native(&old)).unwrap();
    let new_plan = scorestream_app::playback::plan_source(&native(&new)).unwrap();
    assert_eq!(format!("{old_plan:?}"), format!("{new_plan:?}"));
}
fn roundtrip(xml: &str) -> ScoreDocument {
    let first = read(xml);
    let sfs = native(&first);
    let mut again = SfsAdapter.read(&sfs).unwrap().value;
    // SFS parser provenance is not semantic score metadata.
    again.metadata.source.clear();
    assert_eq!(first, again);
    let exported = MusicXml.write(&again).unwrap().value;
    assert_eq!(read(&exported), first);
    first
}
fn tied() -> String {
    STUDY.replace("<staccato placement=\"above\"/>", "<tenuto/>")
        .replacen("<duration>1</duration><voice>1</voice><type>quarter</type><notations><articulations><tenuto/></articulations>", "<duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type><notations><tied type=\"start\"/><articulations><tenuto/></articulations>", 1)
        .replace("<pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>", "<pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/>")
        .replace("<notations><articulations><staccatissimo", "<notations><tied type=\"stop\"/><articulations><staccatissimo")
}

#[test]
fn canonical_four_signs_and_tied_release_keep_exact_legacy_model_and_sound() {
    compatible(STUDY);
    compatible(
        &include_str!("fixtures/articulation-duet.musicxml").replace(
            "<accent/><staccato placement=\"above\"/>",
            "<staccato placement=\"above\"/><accent/>",
        ),
    );
    compatible(&tied());
}
#[test]
fn canonical_fermata_and_slur_combinations_keep_legacy_model() {
    for xml in [
        STUDY.replace(
            "<articulations><tenuto/>",
            "<fermata/><articulations><tenuto/>",
        ),
        STUDY
            .replace(
                "<articulations><staccato",
                "<slur type=\"start\"/><articulations><staccato",
            )
            .replace(
                "<articulations><accent",
                "<slur type=\"stop\"/><articulations><accent",
            ),
    ] {
        assert_eq!(read(&xml), MusicXml.read(&xml).unwrap().value);
        assert!(read(&xml).notation_order.is_none());
    }
}
#[test]
fn automatic_is_explicit_and_other_profiles_remain_unchanged() {
    assert_eq!(
        MusicXmlOptions::default().articulations,
        ArticulationProfile::LegacySimple
    );
    assert_eq!(
        serde_json::to_value(options()).unwrap()["articulations"],
        "automatic"
    );
    let xml = include_str!("fixtures/mixed-authored-notations.musicxml");
    assert!(MusicXml.read(xml).is_err());
    let authored = MusicXmlOptions {
        articulations: ArticulationProfile::AuthoredV1,
        ..Default::default()
    };
    assert!(musicxml::read_with_options(xml, &authored).is_err());
    let mixed = MusicXmlOptions {
        articulations: ArticulationProfile::AuthoredMixedV1,
        ..Default::default()
    };
    assert_eq!(
        roundtrip(xml),
        musicxml::read_with_options(xml, &mixed).unwrap().value
    );
}
#[test]
fn successful_legacy_parse_is_not_a_losslessness_proof() {
    // Legacy accepts both but merges wrappers and sorts signs on output.
    for replacement in [
        "<notations><articulations><accent/><staccato/></articulations></notations>",
        "<notations><articulations><staccato/></articulations></notations><notations><articulations><accent/></articulations></notations>",
    ] {
        let xml = STUDY.replacen(
            "<notations><articulations><staccato placement=\"above\"/></articulations></notations>",
            replacement,
            1,
        );
        assert!(MusicXml.read(&xml).is_ok());
        let new = roundtrip(&xml);
        assert!(new.authored_articulations.is_some());
        assert_eq!(new.articulations.len(), 3, "other notes remain legacy");
        assert!(
            scorestream_app::playback::plan_source(&native(&new)).is_err(),
            "retention is not sound readiness"
        );
    }
}
#[test]
fn rich_empty_repeated_and_qualified_groups_preserve_without_affecting_other_notes() {
    for replacement in [
        "<articulations/>",
        "<articulations id=\"a\"><staccato/></articulations>",
        "<articulations><breath-mark>tick</breath-mark></articulations>",
        "<articulations><staccato default-x=\"1.5\"/></articulations>",
        "<articulations><staccato/><staccato/></articulations>",
        "<articulations><staccato/></articulations><articulations><accent/></articulations>",
    ] {
        let xml = STUDY.replacen(
            "<articulations><staccato placement=\"above\"/></articulations>",
            replacement,
            1,
        );
        let new = roundtrip(&xml);
        assert_eq!(new.authored_articulations.as_ref().unwrap().events.len(), 1);
        assert_eq!(new.articulations.len(), 3);
    }
}
#[test]
fn noncanonical_legacy_accepted_mixed_note_retains_order_and_reports_sound_limit() {
    let xml = tied().replacen(
        "<tied type=\"start\"/><articulations><tenuto/></articulations>",
        "<articulations><tenuto/></articulations><tied type=\"start\"/>",
        1,
    );
    let old = MusicXml.read(&xml).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&native(&old)).is_ok());
    let converted = musicxml::read_with_options(&xml, &options()).unwrap();
    assert!(
        converted
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.authored_only")
    );
    let new = roundtrip(&xml);
    assert!(new.notation_order.is_some());
    assert_eq!(new.ties, old.ties);
    assert!(scorestream_app::playback::plan_source(&native(&new)).is_err());
}
#[test]
fn invalid_and_unimplemented_input_still_returns_source_diagnostics() {
    for (old, new) in [
        (
            "<staccato placement=\"above\"/>",
            "<staccato placement=\"sideways\"/>",
        ),
        (
            "<staccato placement=\"above\"/>",
            "<staccato invented=\"yes\"/>",
        ),
        ("<staccato placement=\"above\"/>", "<unknown/>"),
        (
            "<articulations><staccato placement=\"above\"/></articulations>",
            "<ornaments><turn/></ornaments>",
        ),
    ] {
        let xml = STUDY.replacen(old, new, 1);
        let report = musicxml::read_with_options(&xml, &options()).unwrap_err();
        assert!(report.has_errors());
        assert!(report.diagnostics.iter().any(|d| d.source.is_some()));
    }
}

#[test]
fn retention_diagnostics_identify_source_note_without_claiming_consumer_support() {
    let xml = STUDY.replace("<staccato placement=\"above\"/>", "<breath-mark/>");
    let result = musicxml::read_with_options(&xml, &options()).unwrap();
    let decisions: Vec<_> = result
        .report
        .diagnostics
        .iter()
        .filter(|d| d.feature == "articulation.automatic_retention")
        .collect();
    assert_eq!(decisions.len(), 1);
    assert!(decisions[0].path.contains("note"));
    assert!(decisions[0].source.is_some());
    assert!(decisions[0].message.contains("preview or playback"));
    assert!(
        !musicxml::read_with_options(STUDY, &options())
            .unwrap()
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.automatic_retention")
    );
}

#[test]
fn streaming_reader_uses_the_same_per_note_decision() {
    use std::io::Cursor;
    for xml in [
        STUDY,
        include_str!("fixtures/mixed-authored-notations.musicxml"),
    ] {
        let buffered =
            musicxml::read_reader_with_options(Cursor::new(xml.as_bytes()), &options()).unwrap();
        let string = musicxml::read_with_options(xml, &options()).unwrap();
        assert_eq!(buffered.value, string.value);
        assert_eq!(buffered.report, string.report);
    }
}

#[test]
fn thousands_of_retained_notes_have_one_exact_nonfatal_receipt() {
    let mut xml = String::from(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name/></score-part></part-list><part id="p">"#,
    );
    for i in 0..2048 {
        xml.push_str(&format!(r#"<measure number="{}"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type><notations><articulations><breath-mark/></articulations></notations></note></measure>"#,i+1));
    }
    xml.push_str("</part></score-partwise>");
    let result = musicxml::read_with_options(&xml, &options()).unwrap();
    assert_eq!(result.value.parts[0].measures.len(), 2048);
    let receipts: Vec<_> = result
        .report
        .diagnostics
        .iter()
        .filter(|d| d.feature == "articulation.automatic_retention")
        .collect();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].occurrences, 2048);
    assert!(receipts[0].source.is_some());
    assert!(!result.report.has_errors());
    assert!(
        !result
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "diagnostics.truncated")
    );
    let serialized = serde_json::to_string(&result.report).unwrap();
    let decoded: scorestream_formats::MappingReport = serde_json::from_str(&serialized).unwrap();
    let mut merged = decoded.clone();
    merged.append(decoded);
    assert_eq!(
        merged
            .diagnostics
            .iter()
            .find(|d| d.feature == "articulation.automatic_retention")
            .unwrap()
            .occurrences,
        4096
    );
    // A late real model failure must still abort, not become accepted preservation.
    let broken=xml.replacen("</part></score-partwise>","<measure number=\"2049\"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note><attributes><key><fifths>2</fifths></key></attributes></measure></part></score-partwise>",1);
    let failed = musicxml::read_with_options(&broken, &options()).unwrap_err();
    assert!(
        failed
            .diagnostics
            .iter()
            .any(|d| d.feature == "attributes.mid_measure")
    );
}
