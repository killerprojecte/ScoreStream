//! A genuine combined import must not turn grace attacks into silence merely
//! because the caller selected an explicit fermata policy.
use scorestream_app::playback::{
    FermataSoundPolicy, discover_source_fermatas, plan_source, plan_source_with_explicit_fermatas,
};
use scorestream_core::{
    fermata_timing::ExplicitFermataPolicy,
    notation::{PerformanceLimits, lower_document, perform_document},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

#[test]
fn imported_grace_and_fermata_require_separate_supported_interpretations() {
    // Authored synthetic fixture, never a filtered original score.
    let xml = r#"<score-partwise version="4.0">
      <part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list>
      <part id="p"><measure id="m" number="1">
        <attributes><divisions>4</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
        <note id="grace"><grace slash="yes"/><pitch><step>D</step><octave>5</octave></pitch><voice>1</voice><type>16th</type></note>
        <note id="main"><pitch><step>G</step><octave>4</octave></pitch><duration>16</duration><voice>1</voice><type>whole</type><notations><fermata type="upright"/></notations></note>
      </measure></part>
    </score-partwise>"#;
    let model = MusicXml.read(xml).unwrap().value;
    assert_eq!(model.graces.len(), 1);
    assert_eq!(model.graces[0].id.0, "grace");
    assert_eq!(model.graces[0].anchor.0, "main");
    assert_eq!(model.fermatas.len(), 1);
    assert_eq!(model.fermatas[0].event.0, "main");
    let source = SfsAdapter.write(&model).unwrap().value;
    let original = source.clone();
    let doc = scorestream_codec::parse(&source).unwrap();
    for capability in ["scorestream.grace@1", "scorestream.fermata@1"] {
        assert!(doc.required_extensions.iter().any(|e| e == capability));
    }
    assert_eq!(SfsAdapter.read(&source).unwrap().value, model);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value,
        model
    );
    assert!(
        lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "grace.performance_unsupported")
    );
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
    assert!(
        plan_source(&source)
            .unwrap_err()
            .contains("grace.performance_unsupported")
    );
    // No occurrence-only discovery or partially qualified playable result is
    // exposed. Grace rejects before policy coverage is evaluated.
    assert!(
        discover_source_fermatas(&source)
            .unwrap_err()
            .contains("grace")
    );
    let error = plan_source_with_explicit_fermatas(
        &source,
        &ExplicitFermataPolicy { groups: vec![] },
        FermataSoundPolicy::SustainSelectedAnchors,
    )
    .unwrap_err();
    assert!(error.contains("grace"), "{error}");
    assert!(!error.contains("incomplete_policy"), "{error}");
    assert_eq!(source, original);
    assert_eq!(scorestream_codec::parse(&source).unwrap(), doc);
}
