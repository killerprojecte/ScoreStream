//! Joint regression for the isolated slur/articulation checkpoint.
use scorestream_core::score::ScoreDocument;
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

// Diagnostic source locations and serialization order are not authored notation.
fn semantics(mut score: ScoreDocument) -> ScoreDocument {
    score.metadata.source.clear();
    for measure in score.parts.iter_mut().flat_map(|part| &mut part.measures) {
        measure.events.sort_by_key(|event| event.id.clone());
    }
    score
}

#[test]
fn authored_slurs_and_simple_articulations_roundtrip_without_unlocking_playback() {
    let original = include_str!("fixtures/articulation-duet.musicxml")
        .replace("<articulations><accent/>", "<slur type=\"start\" number=\"1\" bezier-x=\"1.25\" placement=\"above\"/><articulations><accent/>")
        .replace("<articulations><staccatissimo/>", "<slur type=\"stop\" number=\"1\" bezier-y=\"-2.5\" orientation=\"under\"/><articulations><staccatissimo/>");
    let score = MusicXml.read(&original).unwrap().value;
    assert_eq!(score.slurs.len(), 1);
    assert_eq!(score.articulations.len(), 4);
    assert_eq!(score.slurs[0].start.event.0, "head");
    assert_eq!(score.slurs[0].end.event.0, "second");
    let typed: ScoreDocument =
        serde_json::from_str(&serde_json::to_string(&score).unwrap()).unwrap();
    assert_eq!(typed, score);
    let sfs = SfsAdapter.write(&typed).unwrap().value;
    assert!(sfs.contains("!requires scorestream.slur@1"));
    let recovered = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(semantics(recovered.clone()), semantics(score.clone()));
    let exported = MusicXml.write(&recovered).unwrap().value;
    assert_eq!(exported.matches("<slur ").count(), 2);
    assert_eq!(exported.matches("<articulations>").count(), 3);
    let imported = MusicXml.read(&exported).unwrap().value;
    assert_eq!(imported.articulations, score.articulations);
    assert_eq!(imported.slurs[0].start, score.slurs[0].start);
    assert_eq!(imported.slurs[0].end, score.slurs[0].end);
    assert_eq!(
        semantics(imported.clone()).parts,
        semantics(score.clone()).parts
    );
    assert_eq!(SfsAdapter.write(&imported).unwrap().value, sfs);
    let error = scorestream_app::playback::plan_source(&sfs).unwrap_err();
    assert!(error.contains("slur.performance_unsupported"), "{error}");
}

#[test]
fn repeated_articulation_containers_remain_outside_the_bounded_profile() {
    // This is a legal MusicXML grouping, not a malformed XML claim. Retain the
    // current profile refusal until a complete grouping/identity model exists.
    let xml = include_str!("fixtures/articulation-study.musicxml").replacen(
        "</articulations>",
        "</articulations><articulations><accent/></articulations>",
        1,
    );
    let report = MusicXml.read(&xml).unwrap_err();
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic.feature == "xml.cardinality"
                && diagnostic.message.contains("Repeated articulations")
                && diagnostic.source.is_some()
        }),
        "{report:?}"
    );
}
