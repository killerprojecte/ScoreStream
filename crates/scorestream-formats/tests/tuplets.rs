use scorestream_core::{score::NotationPhase, tuplets::TupletChild};
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
#[test]
fn endpoints_bracket_presence_and_exact_rhythm_are_independent() {
    for start in [
        "<tuplet type=\"start\"/>",
        "<tuplet type=\"start\" bracket=\"no\"/>",
        "<tuplet type=\"start\" bracket=\"yes\"/>",
    ] {
        let first = format!("<notations>{start}</notations>");
        let input = xml([&first, "", "<notations><tuplet type=\"stop\"/></notations>"]);
        let score = read(&input);
        let mut plain = score.clone();
        plain.tuplets.clear();
        assert_eq!(
            scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value)
                .unwrap(),
            scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                .unwrap()
        );
        let recovered = SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value;
        assert_eq!(score.tuplets, recovered.tuplets);
        assert_eq!(score.parts, recovered.parts);
        let again = read(&MusicXml.write(&recovered).unwrap().value);
        assert_eq!(score.tuplets, again.tuplets);
        assert_eq!(
            score.parts[0].measures[0].events,
            again.parts[0].measures[0].events
        );
    }
}
#[test]
fn open_and_repeated_endpoints_are_retained_without_invented_pairing() {
    let input = xml([
        "<notations><tuplet type=\"stop\"/><tuplet type=\"start\" bracket=\"no\"/><tuplet type=\"start\"/></notations>",
        "",
        "",
    ]);
    let score = read(&input);
    assert_eq!(score.tuplets[0].children.len(), 3);
    assert!(matches!(
        score.tuplets[0].children[0],
        TupletChild::Tuplet {
            phase: NotationPhase::Stop,
            bracket: None
        }
    ));
    let native = SfsAdapter.write(&score).unwrap().value;
    let recovered = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(score.tuplets, recovered.tuplets);
    let written = MusicXml.write(&recovered).unwrap().value;
    assert!(written.contains(
        "<tuplet type=\"stop\"/><tuplet type=\"start\" bracket=\"no\"/><tuplet type=\"start\"/>"
    ));
}
#[test]
fn canonical_articulation_stays_playable_and_exact_child_order_survives() {
    let input = xml([
        "<notations><articulations><staccato/></articulations><tuplet type=\"start\" bracket=\"no\"/></notations>",
        "",
        "<notations><tuplet type=\"stop\"/></notations>",
    ]);
    let score = read(&input);
    assert!(score.authored_articulations.is_none());
    let mut plain = score.clone();
    plain.tuplets.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    let written = MusicXml
        .write(
            &SfsAdapter
                .read(&SfsAdapter.write(&score).unwrap().value)
                .unwrap()
                .value,
        )
        .unwrap()
        .value;
    assert!(written.contains(
        "<articulations><staccato/></articulations><tuplet type=\"start\" bracket=\"no\"/>"
    ));
    assert_eq!(score.tuplets, read(&written).tuplets);
}
#[test]
fn mixed_authored_articulation_and_slur_references_preserve_wrapper_order() {
    let input = xml([
        "<notations><tuplet type=\"start\" bracket=\"yes\"/><articulations><staccato/><accent/></articulations><slur type=\"start\" number=\"1\"/></notations>",
        "",
        "<notations><slur type=\"stop\" number=\"1\"/><tuplet type=\"stop\"/></notations>",
    ]);
    let score = read(&input);
    assert!(score.authored_articulations.is_some());
    let native = SfsAdapter.write(&score).unwrap().value;
    let recovered = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(score.tuplets, recovered.tuplets);
    let written = MusicXml.write(&recovered).unwrap().value;
    let again = read(&written);
    assert_eq!(score.tuplets, again.tuplets);
    assert_eq!(score.authored_articulations, again.authored_articulations);
    let mut missing = score.clone();
    missing.tuplets[0]
        .children
        .retain(|c| matches!(c, TupletChild::Tuplet { .. }));
    assert!(SfsAdapter.write(&missing).is_err());
}
#[test]
fn wider_shapes_and_native_corruption_do_not_silently_flatten() {
    for n in [
        "<notations><tuplet type=\"start\" number=\"2\"/></notations>",
        "<notations><tuplet type=\"continue\"/></notations>",
        "<notations><tuplet type=\"stop\" bracket=\"no\"/></notations>",
        "<notations><tuplet type=\"start\"><tuplet-actual/></tuplet></notations>",
        "<notations><tuplet type=\"start\"/><ornaments><trill-mark/></ornaments></notations>",
        "<notations><tuplet type=\"start\"/></notations><notations/>",
    ] {
        assert!(
            musicxml::read_with_options(&xml([n, "", ""]), &options()).is_err(),
            "{n}"
        );
    }
    let score = read(&xml([
        "<notations><tuplet type=\"start\"/></notations>",
        "",
        "",
    ]));
    let native = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&native).unwrap();
    let mut bad = doc.clone();
    bad.required_extensions.clear();
    assert!(
        scorestream_core::validate(&bad, None)
            .iter()
            .any(|d| d.code == "tuplets.wire")
    );
    for (key, value) in [
        ("index", scorestream_core::Scalar::Integer(1)),
        (
            "relation",
            scorestream_core::Scalar::String("foreign".into()),
        ),
        ("phase", scorestream_core::Scalar::String("continue".into())),
    ] {
        let mut bad = doc.clone();
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let scorestream_core::Record::Mark(m) = r {
                    if m.kind == scorestream_core::tuplets::KIND {
                        m.attributes.insert(key.into(), value.clone());
                    }
                }
            }
        }
        assert!(scorestream_core::tuplets::decode(&bad).is_err());
    }
    let mut bad = doc.clone();
    for f in &mut bad.frames {
        for r in &mut f.records {
            if let scorestream_core::Record::Mark(m) = r {
                if m.kind == scorestream_core::tuplets::KIND {
                    m.target = "#n1".into();
                }
            }
        }
    }
    assert!(scorestream_core::tuplets::decode(&bad).is_err());
}

#[test]
fn ordinary_tie_sibling_keeps_its_endpoint_and_audio() {
    let input = xml([
        "<notations><tied type=\"start\"/><tuplet type=\"start\" bracket=\"no\"/></notations>",
        "<notations><tied type=\"stop\"/></notations>",
        "<notations><tuplet type=\"stop\"/></notations>",
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
