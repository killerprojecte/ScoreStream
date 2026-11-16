use scorestream_core::{
    score::NotationPhase,
    tuplets::{Ornament, TupletChild},
};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};
fn xml(notations: [&str; 3]) -> String {
    let notes:String=notations.into_iter().enumerate().map(|(i,n)|format!(r#"<note id="n{i}"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>{n}</note>"#)).collect();
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="P"><measure id="m" number="1"><attributes><divisions>3</divisions><time><beats>1</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        articulations: ArticulationProfile::Automatic,
        ..Default::default()
    }
}
fn read(input: &str) -> scorestream_core::score::ScoreDocument {
    musicxml::read_with_options(input, &options())
        .unwrap()
        .value
}
fn roundtrip(n: &str) -> scorestream_core::score::ScoreDocument {
    let input = xml([n, "", ""]);
    let imported = musicxml::read_with_options(&input, &options()).unwrap();
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "ornaments.authored")
    );
    let score = imported.value;
    let native = SfsAdapter.write(&score).unwrap().value;
    assert!(native.contains("scorestream.ornaments@1"));
    let recovered = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(score.tuplets, recovered.tuplets);
    let written = MusicXml.write(&recovered).unwrap().value;
    assert!(written.contains(n), "{written}");
    assert_eq!(score.tuplets, read(&written).tuplets);
    assert_eq!(
        score.parts[0].measures[0].events,
        read(&written).parts[0].measures[0].events
    );
    let mut plain = score.clone();
    plain.tuplets.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&native).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    score
}
#[test]
fn exact_groups_and_mixed_tuplet_positions_are_passive() {
    for n in [
        r#"<notations><ornaments><trill-mark/><tremolo type="single">4</tremolo></ornaments></notations>"#,
        r#"<notations><tuplet type="stop"/><tuplet type="start" bracket="no"/><ornaments><tremolo type="single">1</tremolo></ornaments></notations>"#,
        r#"<notations><tuplet type="stop"/><tuplet type="start" bracket="no"/><articulations><staccato/></articulations><ornaments><tremolo type="single">1</tremolo></ornaments></notations>"#,
        r#"<notations><ornaments><trill-mark/></ornaments><tuplet type="stop"/><ornaments><tremolo type="single">2</tremolo><tremolo type="single">3</tremolo></ornaments></notations>"#,
    ] {
        let score = roundtrip(n);
        assert!(score.authored_articulations.is_none());
    }
}
#[test]
fn wavy_presence_zero_exact_decimal_and_unpaired_sequences() {
    for n in [
        r#"<notations><ornaments><wavy-line type="stop"/><wavy-line type="start" number="1" default-y="0"/><wavy-line type="start" number="1" default-y="-33.84"/></ornaments></notations>"#,
        r#"<notations><ornaments><trill-mark/><wavy-line type="start" number="1" default-y="12.5"/><wavy-line type="stop" number="1"/></ornaments></notations>"#,
    ] {
        roundtrip(n);
    }
    let score =
        roundtrip(r#"<notations><ornaments><wavy-line type="stop"/></ornaments></notations>"#);
    assert!(matches!(
        &score.tuplets[0].children[0],
        TupletChild::Ornament {
            ornament: Ornament::WavyLine {
                phase: NotationPhase::Stop,
                number: None,
                default_y: None
            },
            ..
        }
    ));
}
#[test]
fn unsupported_semantics_and_hierarchy_are_not_discarded() {
    for n in [
        "<notations><ornaments/></notations>",
        "<notations><ornaments><turn/></ornaments></notations>",
        "<notations><ornaments><tremolo type=\"start\">2</tremolo></ornaments></notations>",
        "<notations><ornaments><tremolo type=\"single\">0</tremolo></ornaments></notations>",
        "<notations><ornaments><tremolo type=\"single\">5</tremolo></ornaments></notations>",
        "<notations><ornaments><trill-mark placement=\"above\"/></ornaments></notations>",
        "<notations><ornaments><wavy-line type=\"continue\"/></ornaments></notations>",
        "<notations><ornaments><wavy-line type=\"start\" number=\"2\"/></ornaments></notations>",
        "<notations><ornaments><wavy-line type=\"start\" default-y=\"1/3\"/></ornaments></notations>",
        "<notations><ornaments><trill-mark/></ornaments></notations><notations><tuplet type=\"stop\"/></notations>",
        "<notations id=\"x\"><ornaments><trill-mark/></ornaments></notations>",
    ] {
        assert!(
            musicxml::read_with_options(&xml([n, "", ""]), &options()).is_err(),
            "{n}"
        );
    }
}
#[test]
fn closed_native_capability_fields_and_topology() {
    let score = read(&xml([
        "<notations><ornaments><tremolo type=\"single\">2</tremolo></ornaments></notations>",
        "",
        "",
    ]));
    let native = SfsAdapter.write(&score).unwrap().value;
    for bad in [
        native.replace("scorestream.ornaments@1", "scorestream.tuplets@1"),
        native.replace("ornament_group=0", "ornament_group=1"),
        native.replace("strokes=2", "strokes=5"),
        native.replace("strokes=2", "strokes=2 bracket=true"),
        native.replace("index=0", "index=1"),
        native.replace("scorestream.ornaments.child", "scorestream.tuplets.child"),
    ] {
        assert_ne!(bad, native);
        assert!(SfsAdapter.read(&bad).is_err(), "{bad}");
    }
    let mut invalid = score.clone();
    invalid.tuplets[0].children.push(TupletChild::Tuplet {
        phase: NotationPhase::Stop,
        bracket: None,
    });
    let duplicate = invalid.tuplets[0].children[0].clone();
    invalid.tuplets[0].children.push(duplicate);
    assert!(SfsAdapter.write(&invalid).is_err());
}

#[test]
fn ordinary_tie_and_ornament_keep_order_and_audio() {
    let input = xml([
        "<notations><tied type=\"start\"/><ornaments><tremolo type=\"single\">1</tremolo></ornaments></notations>",
        "<notations><tied type=\"stop\"/></notations>",
        "<notations><ornaments><wavy-line type=\"stop\"/></ornaments></notations>",
    ]);
    let input = input.replacen(
        "<duration>1</duration>",
        "<duration>1</duration><tie type=\"start\"/>",
        1,
    );
    let input=input.replace("<note id=\"n1\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>","<note id=\"n1\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/>");
    let score = read(&input);
    let mut plain = score.clone();
    plain.tuplets.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    let recovered = SfsAdapter
        .read(&SfsAdapter.write(&score).unwrap().value)
        .unwrap()
        .value;
    let again = read(&MusicXml.write(&recovered).unwrap().value);
    assert_eq!(score.tuplets, again.tuplets);
    assert_eq!(score.ties, again.ties);
}
