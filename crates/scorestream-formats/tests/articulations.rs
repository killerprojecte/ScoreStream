use scorestream_core::{
    notation::{DirectionKind, Placement, Scope},
    score::*,
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

const STUDY: &str = include_str!("fixtures/articulation-study.musicxml");
const DUET: &str = include_str!("fixtures/articulation-duet.musicxml");
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn source(xml: &str) -> String {
    SfsAdapter
        .write(&MusicXml.read(xml).unwrap().value)
        .unwrap()
        .value
}
fn roundtrip(xml: &str) -> ScoreDocument {
    let original = MusicXml.read(xml).unwrap().value;
    let json = serde_json::to_string(&original).unwrap();
    let decoded: ScoreDocument = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, original);
    let sfs = SfsAdapter.write(&decoded).unwrap().value;
    let common = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(common.articulations, original.articulations);
    let exported = MusicXml.write(&common).unwrap().value;
    let again = MusicXml.read(&exported).unwrap().value;
    assert_eq!(again.articulations, original.articulations);
    assert_eq!(SfsAdapter.write(&again).unwrap().value, sfs);
    again
}
fn error(xml: &str, feature: &str) {
    let err = MusicXml.read(xml).unwrap_err();
    assert!(
        err.diagnostics.iter().any(|d| d.feature == feature),
        "{feature}: {err:?}"
    );
}
#[test]
fn every_simple_symbol_and_placement_roundtrips_without_foreign_payloads() {
    let model = roundtrip(STUDY);
    assert_eq!(model.articulations.len(), 4);
    for (a, (id, value, place)) in model.articulations.iter().zip([
        (
            "short",
            SimpleArticulation::Staccato,
            Some(Placement::Above),
        ),
        (
            "shortest",
            SimpleArticulation::Staccatissimo,
            Some(Placement::Below),
        ),
        ("held", SimpleArticulation::Tenuto, None),
        ("attack", SimpleArticulation::Accent, Some(Placement::Above)),
    ]) {
        assert_eq!(a.event.0, id);
        assert_eq!(a.value, value);
        assert_eq!(a.placement, place);
    }
    let sfs = source(STUDY);
    assert!(!sfs.contains("scorestream.articulation")); // Existing native semantics, no opaque extension.
    let doc = scorestream_codec::parse(&sfs).unwrap();
    let resolved = scorestream_core::notation::lower_document(&doc).unwrap();
    for d in &resolved.directions {
        if let DirectionKind::Articulation { .. } = d.kind {
            let Scope::Event(id) = &d.scope else {
                panic!("not event-local")
            };
            let event = resolved.events.iter().find(|e| e.id == *id).unwrap();
            assert_eq!(d.at, event.start);
        }
    }
}
#[test]
fn imported_symbols_activate_existing_sound_policy_without_changing_written_clock() {
    let p = scorestream_app::playback::plan_source(&source(STUDY)).unwrap();
    assert_eq!(p.notes.len(), 4);
    for (index, (duration, velocity)) in [(0.25, 80), (0.125, 80), (0.5, 80), (0.46, 96)]
        .into_iter()
        .enumerate()
    {
        close(p.notes[index].start_seconds, index as f64 * 0.5);
        close(p.notes[index].duration_seconds, duration);
        assert_eq!(p.notes[index].velocity, velocity);
    }
    close(p.duration_seconds, 2.);
}
#[test]
fn alternate_duet_keeps_combined_marks_chord_members_voices_and_staves_independent() {
    let model = roundtrip(DUET);
    assert_eq!(model.articulations.len(), 4);
    let p = scorestream_app::playback::plan_source(&source(DUET)).unwrap();
    let note = |id: &str| p.notes.iter().find(|n| n.id == id).unwrap();
    close(note("head").duration_seconds, 0.25);
    assert_eq!(note("head").velocity, 96);
    close(note("member").duration_seconds, 0.5);
    assert_eq!(note("member").velocity, 80);
    close(note("bass").duration_seconds, 0.92);
    assert_eq!(note("bass").velocity, 80);
    close(note("second").duration_seconds, 0.125);
    assert_eq!(note("second").velocity, 80);
    let events: Vec<_> = model
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .collect();
    assert_eq!(events.iter().find(|e| e.id.0 == "bass").unwrap().staff, 2);
    assert_eq!(
        events
            .iter()
            .find(|e| e.id.0 == "member")
            .unwrap()
            .offset
            .to_string(),
        "0"
    );
}
#[test]
fn identical_signs_reject_instead_of_collapsing_but_independent_order_is_canonical() {
    error(
        &STUDY.replace("<staccato placement=\"above\"/>", "<staccato/><staccato/>"),
        "articulation.duplicate",
    );
    let a = DUET.replace("<accent/><staccato", "<staccato/><accent/><staccato");
    error(&a, "articulation.duplicate");
    let reversed = DUET.replace(
        "<accent/><staccato placement=\"above\"/>",
        "<staccato placement=\"above\"/><accent/>",
    );
    assert_eq!(source(&reversed), source(DUET));
}
#[test]
fn unsupported_shapes_and_layout_are_explicit_errors_with_source_context() {
    for shape in [
        "strong-accent",
        "breath-mark",
        "detached-legato",
        "spiccato",
        "scoop",
        "plop",
        "doit",
        "falloff",
        "caesura",
        "stress",
        "unstress",
        "soft-accent",
        "other-articulation",
    ] {
        let xml = STUDY.replace("<staccato placement=\"above\"/>", &format!("<{shape}/>"));
        let e = MusicXml.read(&xml).unwrap_err();
        assert!(
            e.diagnostics
                .iter()
                .any(|d| d.feature == format!("element.{shape}") && d.source.is_some()),
            "{shape}: {e:?}"
        );
    }
    for attribute in [
        "default-x=\"2\"",
        "default-y=\"10\"",
        "relative-x=\"1\"",
        "relative-y=\"2\"",
        "color=\"#000000\"",
        "smufl=\"articStaccatoAbove\"",
        "type=\"down\"",
    ] {
        let xml = STUDY.replace(
            "<staccato placement=\"above\"/>",
            &format!("<staccato {attribute}/>"),
        );
        assert!(MusicXml.read(&xml).is_err(), "{attribute}");
    }
    error(
        &STUDY.replace("placement=\"above\"", "placement=\"sideways\""),
        "articulation.placement",
    );
    error(
        &STUDY.replace("<staccato placement=\"above\"/>", ""),
        "articulation.empty",
    );
    assert!(
        MusicXml
            .read(&STUDY.replace(
                "<staccato placement=\"above\"/>",
                "<staccato>text</staccato>"
            ))
            .is_err()
    );
}
#[test]
fn rest_grace_missing_event_and_duplicate_common_model_anchors_reject() {
    error(
        &STUDY.replacen(
            "<pitch><step>C</step><octave>4</octave></pitch>",
            "<rest/>",
            1,
        ),
        "articulation.event",
    );
    let grace = STUDY.replacen("<duration>1</duration>", "<grace/>", 1);
    assert!(MusicXml.read(&grace).is_err());
    let mut model = MusicXml.read(STUDY).unwrap().value;
    model.articulations[0].event.0 = "missing".into();
    for result in [MusicXml.write(&model), SfsAdapter.write(&model)] {
        assert!(
            result
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "articulation.event")
        );
    }
    model = MusicXml.read(STUDY).unwrap().value;
    model.articulations.push(model.articulations[0].clone());
    assert!(
        MusicXml
            .write(&model)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.duplicate")
    );
}
#[test]
fn sfs_outside_subset_and_unmapped_attributes_are_not_dropped() {
    let s = source(STUDY);
    for shape in ["strong-accent", "breath-mark"] {
        assert!(
            SfsAdapter
                .read(&s.replace("value=staccato", &format!("value={shape}")))
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "articulation.value")
        );
    }
    assert!(
        SfsAdapter
            .read(&s.replace("value=staccato", "id=mark value=staccato"))
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "articulation.attribute")
    );
}
#[test]
fn numeric_dynamics_gate_marks_work_but_attack_conflicts_keep_the_existing_refusal() {
    let xml=STUDY.replace("<note id=\"short\">","<direction><direction-type><dynamics><p/></dynamics></direction-type><sound dynamics=\"50\"/></direction><note id=\"short\">");
    let e = scorestream_app::playback::plan_source(&source(&xml)).unwrap_err();
    assert!(e.contains("attack-changing accents"), "{e}");
    let no_accent = xml.replace(
        "<accent placement=\"above\"/>",
        "<tenuto placement=\"above\"/>",
    );
    let p = scorestream_app::playback::plan_source(&source(&no_accent)).unwrap();
    assert!(p.notes.iter().all(|n| n.velocity == 45));
    close(p.notes[0].duration_seconds, 0.25);
}
#[test]
fn final_tied_articulation_shapes_one_release_without_retriggering() {
    let xml=STUDY.replace("<staccato placement=\"above\"/>","<tenuto/>")
        .replacen("<duration>1</duration><voice>1</voice><type>quarter</type><notations><articulations><tenuto/></articulations>", "<duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type><notations><tied type=\"start\"/><articulations><tenuto/></articulations>", 1)
        .replace("<pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>", "<pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/>")
        .replace("<notations><articulations><staccatissimo", "<notations><tied type=\"stop\"/><articulations><staccatissimo");
    let model = roundtrip(&xml);
    assert_eq!(model.ties.len(), 1);
    let p = scorestream_app::playback::plan_source(&source(&xml)).unwrap();
    assert_eq!(p.notes.len(), 3);
    assert_eq!(p.notes[0].id, "short");
    close(p.notes[0].duration_seconds, 0.625);
}
#[test]
fn old_common_json_remains_compatible_and_unknown_articulation_shapes_are_typed_errors() {
    let model = MusicXml.read(STUDY).unwrap().value;
    let mut json = serde_json::to_value(model).unwrap();
    json.as_object_mut().unwrap().remove("articulations");
    assert!(
        serde_json::from_value::<ScoreDocument>(json.clone())
            .unwrap()
            .articulations
            .is_empty()
    );
    json["articulations"] =
        serde_json::json!([{"event":"short","value":"spiccato","placement":null}]);
    assert!(serde_json::from_value::<ScoreDocument>(json).is_err());
}

#[test]
fn desktop_snapshots_are_generated_from_the_same_musicxml_roundtrip() {
    for (name, xml) in [("articulation-study", STUDY), ("articulation-duet", DUET)] {
        let sfs = source(xml);
        let workspace = scorestream_app::Workspace::from_source(&sfs).unwrap();
        let actual = serde_json::to_value(workspace.current()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../desktop/tests/fixtures/{name}.json"));
        if std::env::var_os("UPDATE_ARTICULATION_FIXTURES").is_some() {
            std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        }
        let checked: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            actual, checked,
            "Regenerate only from the untouched MusicXML fixture with UPDATE_ARTICULATION_FIXTURES=1"
        );
    }
}
