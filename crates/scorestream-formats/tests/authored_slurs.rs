//! Cross-format fidelity and refusal-to-play checks. Fixtures are synthetic.
use scorestream_core::{
    Record, Scalar,
    notation::{EventId, RelationId},
    score::*,
};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::MusicXml,
    sfs::{self, SfsAdapter},
};

fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute 1</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, extra: &str, slurs: &str) -> String {
    format!(
        r#"<note id="{id}">{extra}<pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type><notations>{slurs}</notations></note>"#
    )
}
fn pair() -> String {
    xml(&(note(
        "a",
        "",
        r#"<slur type="start" number="1" bezier-x="+12.500" bezier-y="-0.125" orientation="under" placement="below"/><slur type="start" number="2" bezier-x="4"/>"#,
    ) + &note(
        "b",
        "",
        r#"<slur type="stop" number="1" bezier-x="-3.75" bezier-y="2.5" orientation="over" placement="above"/><slur type="stop" number="2" bezier-y="0"/>"#,
    )))
}
fn xml_semantics(mut score: ScoreDocument) -> ScoreDocument {
    // XML local numbers identify overlap, not persistent relation IDs.
    for slur in &mut score.slurs {
        slur.id = RelationId(String::new());
    }
    score
        .slurs
        .sort_by_key(|s| serde_json::to_string(s).unwrap());
    score
}
fn roundtrip(source: &str) -> (ScoreDocument, String) {
    let score = MusicXml.read(source).unwrap().value;
    let sfs = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&sfs).unwrap();
    let decoded: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    let canonical = scorestream_codec::canonical(&decoded);
    let mapped = SfsAdapter.read(&canonical).unwrap().value;
    assert_eq!(mapped, score);
    let typed: ScoreDocument =
        serde_json::from_str(&serde_json::to_string(&score).unwrap()).unwrap();
    assert_eq!(typed, score);
    let exported = MusicXml.write(&mapped).unwrap().value;
    let recovered = MusicXml.read(&exported).unwrap().value;
    assert_eq!(xml_semantics(recovered), xml_semantics(score.clone()));
    (score, canonical)
}
#[test]
fn independent_endpoint_fields_and_parallel_identity_survive_all_codecs() {
    let (score, source) = roundtrip(&pair());
    assert_eq!(score.slurs.len(), 2);
    assert_ne!(score.slurs[0].id, score.slurs[1].id);
    assert_eq!(score.slurs[0].start.event, score.slurs[1].start.event);
    assert_eq!(score.slurs[0].end.event, score.slurs[1].end.event);
    let s = &score.slurs[0];
    assert_eq!(s.start.bezier_x, Some("25/2".parse().unwrap()));
    assert_eq!(s.start.bezier_y, Some("-1/8".parse().unwrap()));
    assert_eq!(s.end.bezier_x, Some("-15/4".parse().unwrap()));
    assert_ne!(s.start.orientation, s.end.orientation);
    assert_ne!(s.start.placement, s.end.placement);
    assert!(source.contains("!requires scorestream.slur@1"));
    assert!(source.contains("bezier_y=-1/8"));
    assert!(score.slurs[1].start.bezier_y.is_none());
    assert!(score.slurs[1].end.bezier_x.is_none());
    assert_eq!(score.slurs[1].end.bezier_y, Some("0".parse().unwrap()));
}
#[test]
fn grace_to_principal_same_onset_keeps_distinct_event_anchors_and_clock() {
    let grace = note(
        "g",
        "<grace slash=\"yes\"/>",
        "<slur type=\"start\" bezier-x=\"1.25\"/>",
    )
    .replace("<duration>4</duration>", "")
    .replace("<type>quarter</type>", "<type>16th</type>");
    let main = note("a", "", "<slur type=\"stop\" bezier-x=\"-1.25\"/>");
    let end = note("b", "", "").replace("<notations></notations>", "");
    let (score, source) = roundtrip(&xml(&(grace + &main + &end)));
    assert_eq!(score.graces.len(), 1);
    assert_eq!(score.slurs[0].start.event, EventId("g".into()));
    assert_eq!(score.slurs[0].end.event, EventId("a".into()));
    assert_eq!(score.graces[0].anchor.0, "a");
    assert_eq!(
        score.parts[0].measures[0].events[0].offset,
        "0".parse().unwrap()
    );
    assert_eq!(score.parts[0].measures[0].length, "2".parse().unwrap());
    let doc = scorestream_codec::parse(&source).unwrap();
    assert!(matches!(
        doc.events()
            .find(|(_, e)| e.id() == Some("g"))
            .unwrap()
            .1
            .duration,
        scorestream_core::Duration::Grace
    ));
    assert!(scorestream_app::playback::plan_source(&source).is_err());
    assert!(scorestream_app::playback::discover_source_graces(&source).is_err());
}
#[test]
fn imported_slurs_never_silently_select_existing_ordinary_legato_policy() {
    let (_, source) = roundtrip(&pair());
    let original = source.clone();
    let doc = scorestream_codec::parse(&source).unwrap();
    assert!(
        scorestream_core::notation::lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "slur.performance_unsupported")
    );
    assert!(
        scorestream_app::playback::plan_source(&source)
            .unwrap_err()
            .contains("slur.performance_unsupported")
    );
    assert!(scorestream_core::notation::perform_document(&doc, Default::default()).is_err());
    assert_eq!(source, original);
    // Native legacy slurs are not retroactively assigned imported-fidelity policy.
    let legacy = "!score 1.1 unit=q\n!part p instrument=flute staves=1\n!voice v part=p staff=1\n@0\n| p m len=2\nv C4:1 id=a\n~ slur old start=#a placement=below\n.\n@1\nv D4:1 id=b\n~ slur old end=#b\n.\n!end at=2\n";
    assert!(scorestream_app::playback::plan_source(legacy).is_ok());
    assert!(SfsAdapter.read(legacy).is_err());
}
#[test]
fn missing_capability_unknown_fields_and_json_never_bypass_validation() {
    let (_, source) = roundtrip(&pair());
    assert!(
        SfsAdapter
            .read(&source.replace("!requires scorestream.slur@1\n", ""))
            .is_err()
    );
    let mut doc = scorestream_codec::parse(&source).unwrap();
    let rel = doc
        .frames
        .iter_mut()
        .flat_map(|f| &mut f.records)
        .find_map(|r| match r {
            Record::Relation(r) if r.kind == "slur" => Some(r),
            _ => None,
        })
        .unwrap();
    rel.attributes
        .insert("foreign_xml".into(), Scalar::String("<slur/>".into()));
    assert!(sfs::from_document(&doc).is_err());
    let doc: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert!(sfs::from_document(&doc).is_err());
    let mut score = serde_json::to_value(MusicXml.read(&pair()).unwrap().value).unwrap();
    score["slurs"][0]["start"]["foreign_xml"] = serde_json::json!("<slur/>");
    assert!(serde_json::from_value::<ScoreDocument>(score).is_err());
}
#[test]
fn foreign_identifiers_normalize_without_merging_links_or_grace_targets() {
    let mut score = MusicXml.read(&pair()).unwrap().value;
    score.parts[0].measures[0].events[0].id.0 = "foreign event".into();
    score.slurs[0].id.0 = "foreign slur".into();
    score.slurs[1].id.0 = "slur1".into();
    for s in &mut score.slurs {
        s.start.event.0 = "foreign event".into();
    }
    let doc = sfs::to_document(&score).unwrap().value;
    let mapped = SfsAdapter
        .read(&scorestream_codec::canonical(&doc))
        .unwrap()
        .value;
    assert_eq!(mapped.slurs.len(), 2);
    assert_ne!(mapped.slurs[0].id, mapped.slurs[1].id);
    assert!(
        mapped
            .slurs
            .iter()
            .all(|s| s.start.event == mapped.parts[0].measures[0].events[0].id)
    );
}
#[test]
fn nonterminating_xml_coordinate_is_rejected_without_rounding() {
    let mut score = MusicXml.read(&pair()).unwrap().value;
    score.slurs[0].start.bezier_x = Some("1/3".parse().unwrap());
    assert!(SfsAdapter.write(&score).is_ok());
    assert!(MusicXml.write(&score).is_err());
}

#[test]
fn authored_slurs_remain_blocked_in_each_qualified_performance_composition() {
    use scorestream_app::playback::{
        discover_source_fermatas, discover_source_graces, discover_source_graces_and_fermatas,
    };
    let plain = pair();
    let grace = r#"<note id="g"><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>16th</type></note>"#;
    let with_grace = plain.replace("<note id=\"a\">", &format!("{grace}<note id=\"a\">"));
    let with_fermata = plain.replacen("</notations>", "<fermata/></notations>", 1);
    let combined = with_grace.replacen("</notations>", "<fermata/></notations>", 1);
    let source = |xml: &str| {
        SfsAdapter
            .write(&MusicXml.read(xml).unwrap().value)
            .unwrap()
            .value
    };
    let grace_source = source(&with_grace);
    let fermata_source = source(&with_fermata);
    let combined_source = source(&combined);
    for error in [
        discover_source_graces(&grace_source).unwrap_err(),
        discover_source_fermatas(&fermata_source).unwrap_err(),
        discover_source_graces_and_fermatas(&combined_source).unwrap_err(),
    ] {
        assert!(error.contains("performance.unresolved_families"), "{error}");
        assert!(error.contains("scorestream.slur@1"), "{error}");
    }
    assert!(combined_source.contains("!requires scorestream.slur@1"));
    assert!(combined_source.contains("!requires scorestream.grace@1"));
    assert!(combined_source.contains("!requires scorestream.fermata@1"));
}
