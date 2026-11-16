use scorestream_core::{Rational, notation::EventId, score::*};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>4</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, duration: &str, pitch: &str, notation: &str) -> String {
    format!(
        r#"<note id="{id}">{pitch}<duration>{duration}</duration><voice>1</voice>{notation}</note>"#
    )
}
const PITCH: &str = "<pitch><step>G</step><octave>4</octave></pitch>";
fn with_sign(sign: &str) -> String {
    score(&note(
        "n",
        "16",
        PITCH,
        &format!("<notations>{sign}</notations>"),
    ))
}
fn read(source: &str) -> ScoreDocument {
    MusicXml.read(source).unwrap().value
}
fn error(source: &str, feature: &str) {
    let e = MusicXml.read(source).unwrap_err();
    assert!(e.diagnostics.iter().any(|d| d.feature == feature), "{e:?}");
}
#[test]
fn authored_shapes_orientation_and_exact_coordinates_roundtrip_without_timing() {
    for shape in [
        "",
        "normal",
        "angled",
        "square",
        "double-angled",
        "double-square",
        "double-dot",
        "half-curve",
        "curlew",
    ] {
        for kind in ["", " type=\"upright\"", " type=\"inverted\""] {
            let sign =
                format!(r#"<fermata{kind} default-y="0.04" relative-y="-7.13">{shape}</fermata>"#);
            let source = with_sign(&sign);
            let a = read(&source);
            let f = &a.fermatas[0];
            assert_eq!(f.event, EventId("n".into()));
            assert_eq!(f.default_y, Some(r("1/25")));
            assert_eq!(f.relative_y, Some(r("-713/100")));
            assert_eq!(f.shape.is_none(), shape.is_empty());
            assert_eq!(f.orientation.is_none(), kind.is_empty());
            let sfs = SfsAdapter.write(&a).unwrap();
            assert!(sfs.value.contains("!requires scorestream.fermata@1"));
            assert!(sfs.value.contains("+ #n scorestream.fermata"));
            assert!(
                sfs.report
                    .diagnostics
                    .iter()
                    .any(|d| d.feature == "fermata.authored_only")
            );
            let b = SfsAdapter.read(&sfs.value).unwrap().value;
            assert_eq!(a, b);
            let xml = MusicXml.write(&b).unwrap();
            let c = read(&xml.value);
            assert_eq!(a, c);
            assert!(
                scorestream_app::playback::plan_source(&sfs.value)
                    .unwrap_err()
                    .contains("fermata.performance_unsupported")
            );
            assert_eq!(a.parts[0].measures[0].events[0].duration, r("4"));
        }
    }
}
#[test]
fn rest_and_tie_stop_anchors_remain_distinct_written_events() {
    let a = format!(
        r#"<note id="start">{PITCH}<duration>4</duration><tie type="start"/><voice>1</voice><notations><tied type="start"/></notations></note>"#
    );
    let b = format!(
        r#"<note id="stop">{PITCH}<duration>4</duration><tie type="stop"/><voice>1</voice><notations><tied type="stop"/><fermata type="upright" relative-y="5"/></notations></note>"#
    );
    let c = note("rest", "8", "<rest/>", "<notations><fermata/></notations>");
    let model = read(&score(&format!("{a}{b}{c}")));
    assert_eq!(
        model
            .fermatas
            .iter()
            .map(|f| f.event.0.as_str())
            .collect::<Vec<_>>(),
        ["stop", "rest"]
    );
    let sfs = SfsAdapter.write(&model).unwrap().value;
    let round = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(round, model);
    let xml = MusicXml.write(&round).unwrap().value;
    assert_eq!(read(&xml), model);
    assert_eq!(
        model.ties[0].events,
        [EventId("start".into()), EventId("stop".into())]
    );
    assert_eq!(model.parts[0].measures[0].events[1].offset, r("1"));
    assert!(scorestream_app::playback::plan_source(&sfs).is_err());
}
#[test]
fn source_like_different_anchor_intervals_are_not_coalesced_or_stretched() {
    // II m123 has equal onsets but unequal note/rest ends. IV m446 has unequal
    // onsets but equal ends. This explicitly synthetic fixture exercises both.
    let base = read(&score(&note(
        "a",
        "16",
        PITCH,
        "<notations><fermata/></notations>",
    )));
    let mut model = base.clone();
    for (name, offset, duration, rest) in [("rest", "0", "1/2", true), ("late", "2", "2", false)] {
        let mut p = base.parts[0].clone();
        p.id.0 = name.into();
        p.instrument_metadata.as_mut().unwrap().source_id = format!("instrument_{name}");
        p.voices[0].id.0 = format!("v{}_1", model.parts.len() + 1);
        p.measures[0].id = format!("m_{name}");
        let e = &mut p.measures[0].events[0];
        e.id.0 = format!("event_{name}");
        e.voice = p.voices[0].id.clone();
        e.offset = r(offset);
        e.duration = r(duration);
        if rest {
            e.pitch = scorestream_core::Pitch::Rest;
        }
        model.parts.push(p);
        let mut f = base.fermatas[0].clone();
        f.event.0 = format!("event_{name}");
        model.fermatas.push(f);
    }
    let sfs = SfsAdapter.write(&model).unwrap().value;
    let round = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(round, model);
    assert_eq!(round.fermatas.len(), 3);
    assert_eq!(read(&MusicXml.write(&round).unwrap().value), model);
    assert!(scorestream_app::playback::plan_source(&sfs).is_err());
}
#[test]
fn omitted_versus_explicit_normal_and_upright_are_preserved() {
    let a = read(&with_sign("<fermata/>"));
    let b = read(&with_sign("<fermata type=\"upright\">normal</fermata>"));
    assert_ne!(a.fermatas, b.fermatas);
    assert_eq!(a.fermatas[0].shape, None);
    assert_eq!(a.fermatas[0].orientation, None);
    assert_eq!(b.fermatas[0].shape, Some(FermataShape::Normal));
    assert_eq!(b.fermatas[0].orientation, Some(FermataOrientation::Upright));
}
#[test]
fn invalid_unknown_and_unmapped_notation_never_disappears() {
    for (sign, feature) in [
        ("<fermata>longer</fermata>", "fermata.shape"),
        ("<fermata> normal </fermata>", "fermata.shape"),
        ("<fermata type=\"above\"/>", "fermata.orientation"),
        ("<fermata relative-y=\"NaN\"/>", "number.decimal"),
        ("<fermata id=\"f\"/>", "attribute.id"),
        ("<fermata color=\"#FFFFFF\"/>", "attribute.color"),
        ("<fermata default-x=\"2\"/>", "attribute.default-x"),
        (
            "<fermata/> <fermata type=\"inverted\"/>",
            "fermata.duplicate",
        ),
        ("<fermata/><slur type=\"start\"/>", "slur.unmatched"),
        (
            "<fermata/><ornaments><turn/></ornaments>",
            "element.turn",
        ),
    ] {
        error(&with_sign(sign), feature);
    }
    error(
        &with_sign("<fermata/>").replace("<notations>", "<notations print-object=\"no\">"),
        "attribute.print-object",
    );
    let bar = score(&format!(
        "{}<barline location=\"right\"><fermata/></barline>",
        note("a", "16", PITCH, "")
    ));
    error(&bar, "element.fermata");
}
#[test]
fn shared_model_and_sfs_fail_closed_for_bad_anchors_fields_and_capability() {
    let model = read(&with_sign("<fermata/>"));
    let sfs = SfsAdapter.write(&model).unwrap().value;
    for source in [
        sfs.replace("!requires scorestream.fermata@1\n", ""),
        sfs.replace("fermata@1", "fermata@2"),
        sfs.replace("+ #n", "+ p"),
        sfs.replace("+ #n", "+ #missing"),
        sfs.replace("scorestream.fermata\n", "scorestream.fermata hold=2\n"),
    ] {
        assert!(SfsAdapter.read(&source).is_err(), "{source}");
    }
    let mut copy = model.clone();
    copy.fermatas[0].event.0 = "missing".into();
    assert!(MusicXml.write(&copy).is_err());
    let mut copy = model.clone();
    copy.parts[0].measures[0].events[0].pitch = scorestream_core::Pitch::Spacer;
    assert!(
        scorestream_formats::validate_score(&copy)
            .diagnostics
            .iter()
            .any(|d| d.feature == "fermata.event")
    );
    let mut copy = model.clone();
    copy.parts[0].measures[0].events[0].pitch = scorestream_core::Pitch::Unpitched("snare".into());
    assert!(!scorestream_formats::validate_score(&copy).has_errors());
    assert!(SfsAdapter.write(&copy).is_err());
    assert!(MusicXml.write(&copy).is_err());
    let mut copy = model.clone();
    copy.fermatas.push(copy.fermatas[0].clone());
    assert!(SfsAdapter.write(&copy).is_err());
    let mut copy = model.clone();
    copy.fermatas[0].default_y = Some(r("1/3"));
    assert!(SfsAdapter.write(&copy).is_ok());
    assert!(MusicXml.write(&copy).is_err());
    let mut json = serde_json::to_value(&model).unwrap();
    json["fermatas"][0]["duration_multiplier"] = serde_json::json!(2);
    assert!(serde_json::from_value::<ScoreDocument>(json).is_err());
}
#[test]
fn scores_without_fermatas_retain_existing_playback() {
    let model = read(&score(&note("n", "16", PITCH, "")));
    assert!(model.fermatas.is_empty());
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert!(!sfs.contains("scorestream.fermata"));
    assert!(scorestream_app::playback::plan_source(&sfs).is_ok());
}
