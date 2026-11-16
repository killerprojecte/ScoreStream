use scorestream_core::{
    beam_extension::{decode, encode, validate_score, EXTENSION, KIND},
    notation::{lower_document, EventId, PartId, VoiceId},
    score::{
        BeamValue, GraceTiming, NoteSize, NoteType, ScoreBeam, ScoreDocument, ScoreEvent,
        ScoreGraceNote, ScoreMeasure, ScorePart, ScoreVoice, StemDirection, TimeModification,
        WrittenDuration,
    },
    validate, Attributes, Document, DocumentKind, Duration, Event, Frame, Mark, Measure, Part,
    Pitch, Rational, Record, Relation, RelationPhase, Scalar, Severity, SourceRange, State, Voice,
};
use serde_json::json;

fn q(s: &str) -> Rational {
    s.parse().unwrap()
}
fn beam(id: &str, number: Option<u8>, value: BeamValue) -> ScoreBeam {
    ScoreBeam {
        event: EventId(id.into()),
        number,
        value,
    }
}
fn beams() -> Vec<ScoreBeam> {
    use BeamValue::*;
    vec![
        beam("n0", None, Begin),
        beam("n1", Some(1), Continue),
        beam("n2", None, Continue),
        beam("n3", Some(1), End),
        beam("n0", Some(2), Begin),
        beam("n1", Some(2), End),
        beam("n2", Some(2), ForwardHook),
        beam("n3", Some(2), BackwardHook),
    ]
}
fn source() -> SourceRange {
    SourceRange::default()
}
fn common() -> ScoreDocument {
    ScoreDocument {
        beams: beams(),
        parts: vec![ScorePart {labels:None,
            id: PartId("p".into()),
            name: None,
            instrument: None,
            instrument_metadata: None,
            staves: 1,
            voices: vec![ScoreVoice {
                id: VoiceId("v".into()),
                staff: 1,
            }],
            measures: vec![ScoreMeasure {
                id: "m".into(),
                label: "1".into(),
                start: q("0"),
                length: q("1"),
                implicit: false,
                states: vec![],
                clef_changes: vec![],
                events: (0..4)
                    .map(|i| ScoreEvent {
                        visible: None,
                        notehead: None,
                        attack_dynamics: None,
                        id: EventId(format!("n{i}")),
                        voice: VoiceId("v".into()),
                        staff: 1,
                        offset: &Rational::from_integer(i) / &q("4"),
                        duration: q("1/4"),
                        pitch: "C4".parse().unwrap(),
                        chord: None,
                        written: Some(WrittenDuration {
                            note_type: NoteType::N16,
                            size: None,
                            dots: 0,
                            time_modification: None,
                        }),
                        stem: None,
                        accidental: None,
                        measure_rest: None,
                    })
                    .collect(),
            }],
        }],
        ..ScoreDocument::default()
    }
}
fn document() -> Document {
    let mut doc = Document::new(DocumentKind::Score);
    doc.required_extensions.push(EXTENSION.into());
    doc.parts.push(Part {
        id: "p".into(),
        instrument: "piano".into(),
        staves: 1,
        display_name: None,
        source: source(),
    });
    doc.voices.push(Voice {
        id: "v".into(),
        part: "p".into(),
        staff: 1,
        source: source(),
    });
    doc.end = q("1");
    for e in &common().parts[0].measures[0].events {
        let mut records = vec![Record::Event(Event {
            voice: "v".into(),
            pitch: e.pitch.clone(),
            duration: Duration::Measured(e.duration.clone()),
            attributes: [
                ("id".into(), Scalar::String(e.id.0.clone())),
                ("w".into(), Scalar::String("16".into())),
            ]
            .into(),
            source: source(),
        })];
        if e.offset == q("0") {
            records.push(Record::Measure(Measure {
                target: "p".into(),
                id: "m".into(),
                length: q("1"),
                attributes: Attributes::new(),
                source: source(),
            }));
        }
        records.extend(
            beams()
                .iter()
                .filter(|b| b.event == e.id)
                .map(|b| Record::Mark(encode(e.id.0.clone(), b))),
        );
        doc.frames.push(Frame {
            time: e.offset.clone(),
            records,
            source: source(),
        });
    }
    doc
}
fn event(doc: &mut Document, index: usize) -> &mut Event {
    doc.frames[index]
        .records
        .iter_mut()
        .find_map(|r| match r {
            Record::Event(e) => Some(e),
            _ => None,
        })
        .unwrap()
}
fn mark(doc: &mut Document, index: usize, number: u8) -> &mut Mark {
    doc.frames[index]
        .records
        .iter_mut()
        .find_map(|r| match r {
            Record::Mark(m) if decode(m).is_ok_and(|b| b.number.unwrap_or(1) == number) => Some(m),
            _ => None,
        })
        .unwrap()
}
fn native_error(doc: &Document, code: &str) -> bool {
    validate(doc, None)
        .iter()
        .any(|d| d.severity == Severity::Error && d.code == code)
}
fn common_error(doc: &ScoreDocument, code: &str) -> bool {
    validate_score(doc)
        .iter()
        .any(|d| d.severity == Severity::Error && d.code == code)
}
fn assert_valid(doc: &Document) {
    let diagnostics = validate(doc, None);
    assert!(
        diagnostics.iter().all(|d| d.severity != Severity::Error),
        "{diagnostics:?}"
    );
}
fn primary_only(doc: &mut Document) {
    for frame in &mut doc.frames {
        frame.records.retain(
            |r| !matches!(r, Record::Mark(m) if decode(m).is_ok_and(|b| b.number == Some(2))),
        );
    }
}

#[test]
fn typed_codec_preserves_lexical_values_and_number_absence() {
    for value in [
        BeamValue::Begin,
        BeamValue::Continue,
        BeamValue::End,
        BeamValue::ForwardHook,
        BeamValue::BackwardHook,
    ] {
        assert_eq!(BeamValue::from_name(value.name()), Some(value));
        for number in [None, Some(1), Some(8)] {
            let original = beam("n", number, value);
            let mark = encode("n".into(), &original);
            assert_eq!(mark.attributes.contains_key("number"), number.is_some());
            let wire_value = match value {
                BeamValue::ForwardHook => "forward-hook",
                BeamValue::BackwardHook => "backward-hook",
                other => other.name(),
            };
            assert_eq!(
                mark.attributes.get("value").and_then(Scalar::as_str),
                Some(wire_value)
            );
            assert!(!wire_value.chars().any(char::is_whitespace));
            assert_eq!(
                serde_json::to_value(&original).unwrap()["value"],
                value.name()
            );
            assert_eq!(decode(&mark).unwrap(), original);
            assert_eq!(
                serde_json::from_value::<ScoreBeam>(serde_json::to_value(&original).unwrap())
                    .unwrap(),
                original
            );
        }
    }
    let old = json!({"metadata":{},"parts":[]});
    let score: ScoreDocument = serde_json::from_value(old.clone()).unwrap();
    assert!(score.beams.is_empty());
    assert_eq!(serde_json::to_value(score).unwrap(), old);
    assert_eq!(
        serde_json::from_value::<ScoreBeam>(json!({"event":"n","value":"begin"}))
            .unwrap()
            .number,
        None
    );
    for value in [
        json!({"event":"n"}),
        json!({"event":"n","value":"forward-hook"}),
        json!({"event":"n","value":"end","fan":"accel"}),
    ] {
        assert!(serde_json::from_value::<ScoreBeam>(value).is_err());
    }
}
#[test]
fn decoder_rejects_unknown_fields_values_types_and_scopes() {
    let valid = encode("n".into(), &beam("n", None, BeamValue::Begin));
    for field in ["id", "fan", "color", "repeater", "default_x", "sound"] {
        let mut m = valid.clone();
        m.attributes
            .insert(field.into(), Scalar::String("x".into()));
        assert_eq!(decode(&m).unwrap_err().code, "beam.field");
    }
    for value in [
        Scalar::Integer(0),
        Scalar::Integer(9),
        Scalar::Integer(-1),
        Scalar::String("1".into()),
        Scalar::Boolean(true),
    ] {
        let mut m = valid.clone();
        m.attributes.insert("number".into(), value);
        assert_eq!(decode(&m).unwrap_err().code, "beam.number");
    }
    for value in [
        Scalar::Integer(1),
        Scalar::Boolean(true),
        Scalar::String("forward hook".into()),
        Scalar::String("backward hook".into()),
        Scalar::String("Begin".into()),
    ] {
        let mut m = valid.clone();
        m.attributes.insert("value".into(), value);
        assert_eq!(decode(&m).unwrap_err().code, "beam.value");
    }
    let mut m = valid.clone();
    m.attributes.clear();
    assert_eq!(decode(&m).unwrap_err().code, "beam.value");
    for target in ["p", "v", "p.s1", "score", "n", "#", "#bad id"] {
        let mut m = valid.clone();
        m.target = target.into();
        assert_eq!(decode(&m).unwrap_err().code, "beam.event");
    }
    let mut m = valid;
    m.kind = "beam".into();
    assert_eq!(decode(&m).unwrap_err().code, "beam.kind");
}
#[test]
fn native_json_and_common_accept_runs_local_hooks_rests_and_graphic_cue_size() {
    let mut c = common();
    c.parts[0].measures[0].events[1].pitch = Pitch::Rest;
    c.parts[0].measures[0].events[2]
        .written
        .as_mut()
        .unwrap()
        .size = Some(NoteSize::Cue);
    assert!(validate_score(&c).is_empty());
    let mut doc = document();
    event(&mut doc, 1).pitch = Pitch::Rest;
    event(&mut doc, 2)
        .attributes
        .insert("note_size".into(), Scalar::String("cue".into()));
    assert_valid(&doc);
    let decoded: Document = serde_json::from_value(serde_json::to_value(&doc).unwrap()).unwrap();
    assert_valid(&decoded);
    let mut plain = decoded.clone();
    plain.required_extensions.clear();
    for f in &mut plain.frames {
        f.records
            .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
    }
    assert_eq!(
        lower_document(&decoded).unwrap(),
        lower_document(&plain).unwrap()
    );
}
#[test]
fn secondary_hook_does_not_invent_a_neighbor_connection() {
    let mut c = common();
    // A forward hook at level 3 on a level-2 end is an authored partial beam,
    // not a connection to the following level-2 hook.
    c.parts[0].measures[0].events[1]
        .written
        .as_mut()
        .unwrap()
        .note_type = NoteType::N32;
    c.beams.push(beam("n1", Some(3), BeamValue::ForwardHook));
    assert!(validate_score(&c).is_empty());
    let mut doc = document();
    event(&mut doc, 1)
        .attributes
        .insert("w".into(), Scalar::String("32".into()));
    doc.frames[1].records.push(Record::Mark(encode(
        "n1".into(),
        &beam("n1", Some(3), BeamValue::ForwardHook),
    )));
    assert_valid(&doc);
}
#[test]
fn capabilities_require_nonempty_full_score_known_target_and_exact_onset() {
    let mut doc = document();
    doc.required_extensions.clear();
    assert!(native_error(&doc, "beam.requirement"));
    doc.required_extensions = vec!["scorestream.beam@2".into()];
    assert!(native_error(&doc, "extension_unsupported"));
    let mut doc = document();
    for f in &mut doc.frames {
        f.records.retain(|r| !matches!(r, Record::Mark(_)));
    }
    assert!(native_error(&doc, "beam.empty_requirement"));
    let mut doc = document();
    doc.document_kind = DocumentKind::Fragment;
    doc.span = Some(doc.end.clone());
    assert!(native_error(&doc, "beam.document"));
    let mut doc = document();
    mark(&mut doc, 0, 1).target = "#missing".into();
    assert!(native_error(&doc, "beam.event"));
    let mut doc = document();
    let moved = Record::Mark(mark(&mut doc, 0, 1).clone());
    doc.frames[1].records.push(moved);
    assert!(native_error(&doc, "beam.onset"));
    let mut doc = document();
    doc.frames[0]
        .records
        .retain(|r| !matches!(r, Record::Measure(_)));
    assert!(native_error(&doc, "beam.measure"));
    let mut c = common();
    c.beams[0].event = EventId("missing".into());
    assert!(common_error(&c, "beam.event"));
    let mut c = common();
    c.parts[0].measures[0].length = q("1/8");
    assert!(common_error(&c, "beam.measure"));
}
#[test]
fn reject_duplicate_effective_levels_missing_lower_levels_and_insufficient_flags() {
    let mut doc = document();
    doc.frames[0].records.push(Record::Mark(encode(
        "n0".into(),
        &beam("n0", Some(1), BeamValue::Begin),
    )));
    assert!(native_error(&doc, "beam.duplicate"));
    let mut c = common();
    c.beams.push(beam("n0", Some(1), BeamValue::Begin));
    assert!(common_error(&c, "beam.duplicate"));
    let mut doc = document();
    doc.frames[0]
        .records
        .retain(|r| !matches!(r,Record::Mark(m) if decode(m).is_ok_and(|b| b.number.is_none())));
    assert!(native_error(&doc, "beam.lower_level"));
    let mut c = common();
    c.beams
        .retain(|b| !(b.event.0 == "n0" && b.number.is_none()));
    assert!(common_error(&c, "beam.lower_level"));
    for written in [None, Some("4"), Some("8")] {
        let mut doc = document();
        event(&mut doc, 0).attributes.remove("w");
        if let Some(w) = written {
            event(&mut doc, 0)
                .attributes
                .insert("w".into(), Scalar::String(w.into()));
        }
        assert!(native_error(&doc, "beam.written"));
    }
    let mut c = common();
    c.parts[0].measures[0].events[0].written = None;
    assert!(common_error(&c, "beam.written"));
    for number in [0, 9, 255] {
        let mut c = common();
        c.beams[0].number = Some(number);
        assert!(common_error(&c, "beam.number"));
    }
}
#[test]
fn reject_primary_hooks_singleton_chords_semantic_cues_unpitched_spacers_and_measure_rests() {
    for value in [BeamValue::ForwardHook, BeamValue::BackwardHook] {
        let mut c = common();
        c.beams[0].value = value;
        assert!(common_error(&c, "beam.primary_hook_unsupported"));
        let mut doc = document();
        mark(&mut doc, 0, 1).attributes = encode("n0".into(), &beam("n0", None, value)).attributes;
        assert!(native_error(&doc, "beam.primary_hook_unsupported"));
    }
    for (key, value) in [
        ("kind", Scalar::String("cue".into())),
        ("measure_rest", Scalar::Boolean(true)),
    ] {
        let mut doc = document();
        event(&mut doc, 0).attributes.insert(key.into(), value);
        assert!(native_error(&doc, "beam.profile"));
    }
    for pitch in [Pitch::Unpitched("snare".into()), Pitch::Spacer] {
        let mut c = common();
        c.parts[0].measures[0].events[0].pitch = pitch.clone();
        assert!(common_error(&c, "beam.profile"));
        let mut doc = document();
        event(&mut doc, 0).pitch = pitch;
        assert!(native_error(&doc, "beam.profile"));
    }
    let mut c = common();
    c.parts[0].measures[0].events[0].chord = Some("c".into());
    assert!(common_error(&c, "beam.chord_group"));
    let mut doc = document();
    event(&mut doc, 0)
        .attributes
        .insert("chord".into(), Scalar::String("c".into()));
    assert!(native_error(&doc, "beam.chord_group"));
}
#[test]
fn reject_orphan_unclosed_and_interrupted_runs_including_anonymous_events() {
    let mut doc = document();
    mark(&mut doc, 0, 1)
        .attributes
        .insert("value".into(), Scalar::String("continue".into()));
    assert!(native_error(&doc, "beam.sequence"));
    let mut doc = document();
    mark(&mut doc, 3, 1)
        .attributes
        .insert("value".into(), Scalar::String("continue".into()));
    assert!(native_error(&doc, "beam.unclosed"));
    let mut doc = document();
    doc.frames[1]
        .records
        .retain(|r| !matches!(r, Record::Mark(_)));
    assert!(native_error(&doc, "beam.interrupted"));
    let mut c = common();
    c.beams.retain(|b| b.event.0 != "n1");
    assert!(common_error(&c, "beam.interrupted"));
    let mut doc = document();
    let mut anonymous = event(&mut doc, 0).clone();
    anonymous.attributes.remove("id");
    // Even a long unmarked event whose tail crosses its measure must not vanish
    // from the beam topology lane merely because its duration is unsupported.
    anonymous.duration = Duration::Measured(q("1"));
    doc.frames.insert(
        1,
        Frame {
            time: q("1/8"),
            records: vec![Record::Event(anonymous)],
            source: source(),
        },
    );
    assert!(native_error(&doc, "beam.interrupted"));
}
#[test]
fn reject_gaps_overlaps_cross_measure_voice_staff_and_lower_level_breaks() {
    for duration in ["1/8", "1/2"] {
        let mut doc = document();
        event(&mut doc, 0).duration = Duration::Measured(q(duration));
        assert!(native_error(&doc, "beam.adjacency"));
        let mut c = common();
        c.parts[0].measures[0].events[0].duration = q(duration);
        assert!(common_error(&c, "beam.adjacency"));
    }
    let mut doc = document();
    let Record::Measure(m) = &mut doc.frames[0].records[1] else {
        unreachable!()
    };
    m.length = q("1/2");
    doc.frames[2].records.push(Record::Measure(Measure {
        target: "p".into(),
        id: "m2".into(),
        length: q("1/2"),
        attributes: Attributes::new(),
        source: source(),
    }));
    assert!(native_error(&doc, "beam.unclosed"));
    assert!(native_error(&doc, "beam.sequence"));
    let mut doc = document();
    doc.voices.push(Voice {
        id: "v2".into(),
        part: "p".into(),
        staff: 1,
        source: source(),
    });
    event(&mut doc, 2).voice = "v2".into();
    assert!(native_error(&doc, "beam.sequence"));
    let mut doc = document();
    doc.parts[0].staves = 2;
    doc.frames[2].records.push(Record::State(State {
        target: "v".into(),
        attributes: [("staff".into(), Scalar::Integer(2))].into(),
        source: source(),
    }));
    assert!(native_error(&doc, "beam.sequence"));
    let mut doc = document();
    // Both primary runs are valid, but a secondary run may not bridge them.
    for (i, value) in [(1, "end"), (2, "begin")] {
        mark(&mut doc, i, 1)
            .attributes
            .insert("value".into(), Scalar::String(value.into()));
    }
    for (i, value) in [(1, "continue"), (2, "continue"), (3, "end")] {
        mark(&mut doc, i, 2)
            .attributes
            .insert("value".into(), Scalar::String(value.into()));
    }
    assert!(native_error(&doc, "beam.lower_connection"));
    let mut c = common();
    c.beams[1].value = BeamValue::End;
    c.beams[2].value = BeamValue::Begin;
    c.beams[5].value = BeamValue::Continue;
    c.beams[6].value = BeamValue::Continue;
    c.beams[7].value = BeamValue::End;
    assert!(common_error(&c, "beam.lower_connection"));
}
#[test]
fn effective_staff_state_is_applied_before_events_and_event_override_wins() {
    let mut doc = document();
    doc.parts[0].staves = 2;
    let Record::Measure(m) = &mut doc.frames[0].records[1] else {
        unreachable!()
    };
    m.target = "p.s2".into();
    // Intentionally after event and beam records at this onset.
    doc.frames[0].records.push(Record::State(State {
        target: "v".into(),
        attributes: [("staff".into(), Scalar::Integer(2))].into(),
        source: source(),
    }));
    assert_valid(&doc);
    event(&mut doc, 0)
        .attributes
        .insert("staff".into(), Scalar::Integer(1));
    assert!(native_error(&doc, "beam.measure"));
}
fn graces() -> Document {
    let mut doc = document();
    primary_only(&mut doc);
    for frame in &mut doc.frames {
        frame.records.retain(|r| matches!(r, Record::Measure(_)));
    }
    doc.frames.truncate(1);
    doc.required_extensions
        .push(scorestream_core::grace_extension::EXTENSION.into());
    doc.frames[0].records.push(Record::Event(Event {
        voice: "v".into(),
        pitch: "C4".parse().unwrap(),
        duration: Duration::Measured(q("1")),
        attributes: [
            ("id".into(), Scalar::String("principal".into())),
            ("w".into(), Scalar::String("4".into())),
        ]
        .into(),
        source: source(),
    }));
    for (index, value) in [BeamValue::Begin, BeamValue::Continue, BeamValue::End]
        .into_iter()
        .enumerate()
    {
        let id = format!("g{index}");
        doc.frames[0].records.push(Record::Event(Event {
            voice: "v".into(),
            pitch: "D4".parse().unwrap(),
            duration: Duration::Grace,
            attributes: [
                ("id".into(), Scalar::String(id.clone())),
                ("w".into(), Scalar::String("8".into())),
                ("kind".into(), Scalar::String("grace".into())),
                ("anchor".into(), Scalar::String("#principal".into())),
                ("order".into(), Scalar::Integer(index as i64 - 3)),
            ]
            .into(),
            source: source(),
        }));
        doc.frames[0].records.push(Record::Mark(Mark {
            target: format!("#{id}"),
            kind: scorestream_core::grace_extension::KIND.into(),
            attributes: Attributes::new(),
            source: source(),
        }));
        doc.frames[0]
            .records
            .push(Record::Mark(encode(id.clone(), &beam(&id, None, value))));
    }
    doc
}
#[test]
fn grace_lanes_require_explicit_valid_before_principal_profile() {
    let doc = graces();
    assert_valid(&doc);
    assert!(lower_document(&doc)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "grace.performance_unsupported"));
    let mut c = common();
    c.beams.clear();
    for (i, value) in [BeamValue::Begin, BeamValue::Continue, BeamValue::End]
        .into_iter()
        .enumerate()
    {
        let id = format!("g{i}");
        c.beams.push(beam(&id, None, value));
        c.graces.push(ScoreGraceNote {
            visible: None,
            notehead: None,
            attack_dynamics: None,
            id: EventId(id),
            anchor: EventId("n0".into()),
            order: i as i64 - 3,
            pitch: "D4".parse().unwrap(),
            written: Some(WrittenDuration {
                note_type: NoteType::Eighth,
                size: None,
                dots: 0,
                time_modification: None,
            }),
            slash: None,
            stem: None,
            accidental: None,
            timing: GraceTiming::default(),
        });
    }
    assert!(validate_score(&c).is_empty());
    c.graces[1].order = -1;
    assert!(common_error(&c, "beam.grace"));
    let mut doc = graces();
    doc.required_extensions
        .retain(|e| e != scorestream_core::grace_extension::EXTENSION);
    assert!(native_error(&doc, "beam.profile"));
    for (key, value) in [
        ("anchor", Scalar::String("#missing".into())),
        ("order", Scalar::Integer(1)),
    ] {
        let mut doc = graces();
        let grace = doc.frames[0]
            .records
            .iter_mut()
            .find_map(|r| match r {
                Record::Event(e) if e.is_grace() => Some(e),
                _ => None,
            })
            .unwrap();
        grace.attributes.insert(key.into(), value);
        assert!(native_error(&doc, "beam.grace"));
    }
    let mut doc = graces();
    doc.frames[0]
        .records
        .retain(|r| !matches!(r,Record::Mark(m) if m.kind==KIND && m.target=="#g1"));
    assert!(native_error(&doc, "beam.interrupted"));
}
#[test]
fn legacy_relation_mixing_rejects_without_weakening_other_lowering_guards() {
    let mut doc = document();
    doc.frames[0].records.push(Record::Relation(Relation {
        kind: "beam".into(),
        id: "legacy".into(),
        phase: RelationPhase::Start,
        anchor: "#n0".into(),
        attributes: [("level".into(), Scalar::Integer(1))].into(),
        source: source(),
    }));
    doc.frames[3].records.push(Record::Relation(Relation {
        kind: "beam".into(),
        id: "legacy".into(),
        phase: RelationPhase::End,
        anchor: "#n3".into(),
        attributes: Attributes::new(),
        source: source(),
    }));
    assert!(native_error(&doc, "beam.legacy_conflict"));
    doc.required_extensions.clear();
    for frame in &mut doc.frames {
        frame
            .records
            .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
    }
    assert_valid(&doc);
    assert_eq!(lower_document(&doc).unwrap().relations.len(), 1);
    let mut doc = document();
    doc.required_extensions
        .push(scorestream_core::fermata_extension::EXTENSION.into());
    doc.frames[0].records.push(Record::Mark(Mark {
        target: "#n0".into(),
        kind: scorestream_core::fermata_extension::KIND.into(),
        attributes: Attributes::new(),
        source: source(),
    }));
    assert_valid(&doc);
    assert!(lower_document(&doc)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "fermata.performance_unsupported"));
    let mut doc = document();
    doc.frames[0].records.push(Record::Mark(Mark {
        target: "#n0".into(),
        kind: "unknown.beam".into(),
        attributes: Attributes::new(),
        source: source(),
    }));
    assert!(native_error(&doc, "kind_unsupported"));
    assert!(lower_document(&doc).is_err());
}

#[test]
fn qualified_projection_keeps_validated_beams_sound_neutral_and_rejects_bad_topology() {
    use scorestream_core::notation::{
        discover_document_fermatas, discover_document_graces,
        discover_document_graces_and_fermatas, perform_document, PerformanceLimits,
    };
    fn without_beams(doc: &Document) -> Document {
        let mut result = doc.clone();
        result.required_extensions.retain(|e| e != EXTENSION);
        for f in &mut result.frames {
            f.records
                .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
        }
        result
    }
    fn add_fermata(doc: &mut Document, event: &str) {
        doc.required_extensions
            .push(scorestream_core::fermata_extension::EXTENSION.into());
        doc.frames[0].records.push(Record::Mark(Mark {
            target: format!("#{event}"),
            kind: scorestream_core::fermata_extension::KIND.into(),
            attributes: Attributes::new(),
            source: source(),
        }));
    }
    let mut doc = document();
    doc.required_extensions
        .push(scorestream_core::navigation_extension::EXTENSION.into());
    doc.frames.push(Frame {
        time: doc.end.clone(),
        records: vec![Record::Mark(Mark {
            target: "p".into(),
            kind: scorestream_core::navigation_extension::KIND.into(),
            attributes: [
                ("measure".into(), Scalar::String("m".into())),
                ("side".into(), Scalar::String("right".into())),
                ("style".into(), Scalar::String("regular".into())),
            ]
            .into(),
            source: source(),
        })],
        source: source(),
    });
    assert_eq!(
        perform_document(&doc, PerformanceLimits::default()).unwrap(),
        perform_document(&without_beams(&doc), PerformanceLimits::default()).unwrap()
    );
    let mut doc = document();
    add_fermata(&mut doc, "n0");
    assert_eq!(
        discover_document_fermatas(&doc).unwrap(),
        discover_document_fermatas(&without_beams(&doc)).unwrap()
    );
    let mut doc = graces();
    assert_eq!(
        discover_document_graces(&doc).unwrap(),
        discover_document_graces(&without_beams(&doc)).unwrap()
    );
    add_fermata(&mut doc, "principal");
    assert_eq!(
        discover_document_graces_and_fermatas(&doc).unwrap(),
        discover_document_graces_and_fermatas(&without_beams(&doc)).unwrap()
    );
    doc.frames[0]
        .records
        .retain(|r| !matches!(r,Record::Mark(m) if m.kind==KIND && m.target=="#g1"));
    assert!(discover_document_graces_and_fermatas(&doc)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "beam.interrupted"));
}

#[test]
fn overlapping_unmarked_events_cannot_hide_before_a_beam_opens() {
    for unmarked_first in [false, true] {
        let mut doc = document();
        let mut other = event(&mut doc, 0).clone();
        other
            .attributes
            .insert("id".into(), Scalar::String("other".into()));
        let index = if unmarked_first {
            0
        } else {
            doc.frames[0].records.len()
        };
        doc.frames[0].records.insert(index, Record::Event(other));
        assert!(
            native_error(&doc, "beam.overlap"),
            "native order {unmarked_first}"
        );
        let mut c = common();
        let mut other = c.parts[0].measures[0].events[0].clone();
        other.id = EventId("other".into());
        let index = if unmarked_first { 0 } else { 1 };
        c.parts[0].measures[0].events.insert(index, other);
        assert!(
            common_error(&c, "beam.overlap"),
            "common order {unmarked_first}"
        );
    }
    let mut doc = document();
    let mut other = event(&mut doc, 0).clone();
    other
        .attributes
        .insert("id".into(), Scalar::String("earlier".into()));
    other.duration = Duration::Measured(q("1/2"));
    // Start the authored run later while a preceding unmarked note still sounds.
    for frame in &mut doc.frames {
        frame.time = &frame.time + &q("1/4");
        frame.records.retain(|r| !matches!(r, Record::Measure(_)));
    }
    doc.end = q("5/4");
    doc.frames.insert(
        0,
        Frame {
            time: q("0"),
            records: vec![
                Record::Event(other),
                Record::Measure(Measure {
                    target: "p".into(),
                    id: "m".into(),
                    length: q("5/4"),
                    attributes: Attributes::new(),
                    source: source(),
                }),
            ],
            source: source(),
        },
    );
    assert!(native_error(&doc, "beam.overlap"));
    let mut c = common();
    let measure = &mut c.parts[0].measures[0];
    let mut other = measure.events[0].clone();
    other.id = EventId("earlier".into());
    other.duration = q("1/2");
    for e in &mut measure.events {
        e.offset = &e.offset + &q("1/4");
    }
    measure.length = q("5/4");
    measure.events.insert(0, other);
    assert!(common_error(&c, "beam.overlap"));
    // Closing the run does not make the last member's sounding tail disappear.
    let mut doc = document();
    let mut tail = event(&mut doc, 3).clone();
    tail.attributes
        .insert("id".into(), Scalar::String("tail".into()));
    tail.duration = Duration::Measured(q("1/8"));
    doc.frames.push(Frame {
        time: q("7/8"),
        records: vec![Record::Event(tail)],
        source: source(),
    });
    assert!(native_error(&doc, "beam.overlap"));
    let mut c = common();
    let mut tail = c.parts[0].measures[0].events[3].clone();
    tail.id = EventId("tail".into());
    tail.offset = q("7/8");
    tail.duration = q("1/8");
    c.parts[0].measures[0].events.push(tail);
    assert!(common_error(&c, "beam.overlap"));
    // The same overlap in another voice is unrelated to this printed beam lane.
    let mut doc = document();
    let mut other = event(&mut doc, 0).clone();
    other.voice = "other_voice".into();
    other.attributes.remove("id");
    other.duration = Duration::Measured(q("1"));
    doc.voices.push(Voice {
        id: "other_voice".into(),
        part: "p".into(),
        staff: 1,
        source: source(),
    });
    doc.frames[0].records.insert(0, Record::Event(other));
    assert_valid(&doc);
}

#[test]
fn all_eight_written_flag_levels_use_the_same_native_and_common_contract() {
    for (index, note_type) in [
        NoteType::Eighth,
        NoteType::N16,
        NoteType::N32,
        NoteType::N64,
        NoteType::N128,
        NoteType::N256,
        NoteType::N512,
        NoteType::N1024,
    ]
    .into_iter()
    .enumerate()
    {
        let maximum = index as u8 + 1;
        let duration = q(&format!("1/{}", 1u32 << maximum));
        let length = &duration + &duration;
        let mut c = common();
        c.beams.clear();
        let measure = &mut c.parts[0].measures[0];
        measure.length = length.clone();
        measure.events.truncate(2);
        for (i, e) in measure.events.iter_mut().enumerate() {
            e.offset = if i == 0 { q("0") } else { duration.clone() };
            e.duration = duration.clone();
            e.written.as_mut().unwrap().note_type = note_type;
            for level in 1..=maximum {
                c.beams.push(beam(
                    &e.id.0,
                    Some(level),
                    if i == 0 {
                        BeamValue::Begin
                    } else {
                        BeamValue::End
                    },
                ));
            }
        }
        assert!(validate_score(&c).is_empty(), "common level {maximum}");
        let mut doc = document();
        doc.end = length.clone();
        doc.frames.truncate(2);
        doc.frames[1].time = duration.clone();
        for (i, frame) in doc.frames.iter_mut().enumerate() {
            frame.records.retain(|r| !matches!(r, Record::Mark(_)));
            for record in &mut frame.records {
                match record {
                    Record::Event(e) => {
                        e.duration = Duration::Measured(duration.clone());
                        e.attributes.insert(
                            "w".into(),
                            Scalar::String((1u32 << (maximum + 2)).to_string()),
                        );
                    }
                    Record::Measure(m) => m.length = length.clone(),
                    _ => (),
                }
            }
            let id = format!("n{i}");
            for level in 1..=maximum {
                frame.records.push(Record::Mark(encode(
                    id.clone(),
                    &beam(
                        &id,
                        Some(level),
                        if i == 0 {
                            BeamValue::Begin
                        } else {
                            BeamValue::End
                        },
                    ),
                )));
            }
        }
        assert_valid(&doc);
    }
}

fn common_event<'a>(score: &'a mut ScoreDocument, id: &str) -> &'a mut ScoreEvent {
    score.parts[0].measures[0]
        .events
        .iter_mut()
        .find(|e| e.id.0 == id)
        .unwrap()
}
fn native_event<'a>(doc: &'a mut Document, id: &str) -> &'a mut Event {
    doc.frames
        .iter_mut()
        .flat_map(|f| &mut f.records)
        .find_map(|r| match r {
            Record::Event(e) if e.id() == Some(id) => Some(e),
            _ => None,
        })
        .unwrap()
}
fn add_common_chord(score: &mut ScoreDocument, id: &str, count: usize) {
    let events = &mut score.parts[0].measures[0].events;
    let index = events.iter().position(|e| e.id.0 == id).unwrap();
    events[index].chord = Some(format!("chord_{id}"));
    for tone in 1..count {
        let mut member = events[index].clone();
        member.id = EventId(format!("{id}_{tone}"));
        member.pitch = if tone == 1 { "E4" } else { "G4" }.parse().unwrap();
        events.insert(index + tone, member);
    }
}
fn add_native_chord(doc: &mut Document, id: &str, count: usize) {
    for frame in &mut doc.frames {
        let Some(index) = frame
            .records
            .iter()
            .position(|r| matches!(r, Record::Event(e) if e.id() == Some(id)))
        else {
            continue;
        };
        let Record::Event(head) = &mut frame.records[index] else {
            unreachable!();
        };
        head.attributes
            .insert("chord".into(), Scalar::String(format!("chord_{id}")));
        let head = head.clone();
        for tone in 1..count {
            let mut member = head.clone();
            member
                .attributes
                .insert("id".into(), Scalar::String(format!("{id}_{tone}")));
            member.pitch = if tone == 1 { "E4" } else { "G4" }.parse().unwrap();
            frame.records.insert(index + tone, Record::Event(member));
        }
        return;
    }
    panic!("missing chord head {id}");
}
fn chord_common() -> ScoreDocument {
    let mut score = common();
    for (id, count) in [("n0", 2), ("n1", 3), ("n2", 2), ("n3", 2)] {
        add_common_chord(&mut score, id, count);
    }
    score
}
fn chord_document() -> Document {
    let mut doc = document();
    for (id, count) in [("n0", 2), ("n1", 3), ("n2", 2), ("n3", 2)] {
        add_native_chord(&mut doc, id, count);
    }
    doc
}

#[test]
fn measured_chord_heads_support_dyads_triads_dots_tuplets_sizes_and_authored_stems() {
    for stem in [None, Some(StemDirection::Up), Some(StemDirection::Down)] {
        for size in [
            None,
            Some(NoteSize::Full),
            Some(NoteSize::Cue),
            Some(NoteSize::GraceCue),
            Some(NoteSize::Large),
        ] {
            for tuplets in [false, true] {
                let mut score = chord_common();
                let mut doc = chord_document();
                for e in &mut score.parts[0].measures[0].events {
                    e.stem = stem;
                    let w = e.written.as_mut().unwrap();
                    w.size = size;
                    w.dots = 1;
                    if tuplets {
                        w.time_modification = Some(TimeModification {
                            actual: 3,
                            normal: 2,
                            normal_type: Some(NoteType::N16),
                            normal_dots: 1,
                        });
                    }
                }
                for e in doc
                    .frames
                    .iter_mut()
                    .flat_map(|f| &mut f.records)
                    .filter_map(|r| {
                        if let Record::Event(e) = r {
                            Some(e)
                        } else {
                            None
                        }
                    })
                {
                    e.attributes.insert("dots".into(), Scalar::Integer(1));
                    if let Some(stem) = stem {
                        e.attributes
                            .insert("stem".into(), Scalar::String(stem.name().into()));
                    }
                    if let Some(size) = size {
                        e.attributes
                            .insert("note_size".into(), Scalar::String(size.name().into()));
                    }
                    if tuplets {
                        for (key, value) in [("tm", "3:2"), ("tm_normal_type", "16")] {
                            e.attributes
                                .insert(key.into(), Scalar::String(value.into()));
                        }
                        e.attributes
                            .insert("tm_normal_dots".into(), Scalar::Integer(1));
                    }
                }
                let before = score.clone();
                assert!(
                    validate_score(&score).is_empty(),
                    "{:?}",
                    validate_score(&score)
                );
                assert_eq!(score, before);
                let before = doc.clone();
                assert_valid(&doc);
                assert_eq!(doc, before);
            }
        }
    }
    // A measured chord and an ordinary note share the same topology rules.
    let mut score = common();
    add_common_chord(&mut score, "n1", 3);
    assert!(validate_score(&score).is_empty());
    let mut doc = document();
    add_native_chord(&mut doc, "n1", 3);
    assert_valid(&doc);
}

#[test]
fn chord_head_is_first_declared_member_even_after_json_reordering() {
    let mut score = chord_common();
    score.parts[0].measures[0].events.swap(0, 1);
    let score: ScoreDocument =
        serde_json::from_value(serde_json::to_value(score).unwrap()).unwrap();
    assert!(common_error(&score, "beam.chord_member"));
    assert_eq!(score.beams[0].event.0, "n0");
    let mut doc = chord_document();
    doc.frames[0].records.swap(0, 1);
    let doc: Document = serde_json::from_value(serde_json::to_value(doc).unwrap()).unwrap();
    assert!(native_error(&doc, "beam.chord_member"));
    // Reordering the beam marks cannot select a different head either.
    let mut doc = chord_document();
    let record = doc.frames[0].records.pop().unwrap();
    doc.frames[0].records.insert(0, record);
    assert_valid(&doc);
}

#[test]
fn chord_members_cannot_carry_or_split_any_beam_level() {
    for split in [false, true] {
        for number in [None, Some(2)] {
            let mut score = chord_common();
            let mut doc = chord_document();
            if split {
                score
                    .beams
                    .retain(|b| !(b.event.0 == "n0" && b.number == number));
                doc.frames[0].records.retain(|r| {
                    !matches!(r, Record::Mark(m) if decode(m).is_ok_and(|b| b.event.0 == "n0" && b.number == number))
                });
            }
            score.beams.push(beam("n0_1", number, BeamValue::Begin));
            doc.frames[0].records.push(Record::Mark(encode(
                "n0_1".into(),
                &beam("n0_1", number, BeamValue::Begin),
            )));
            assert!(common_error(&score, "beam.chord_member"));
            assert!(native_error(&doc, "beam.chord_member"));
        }
    }
}

#[test]
fn chord_members_require_full_equal_explicit_written_rhythm_and_duration() {
    for mutation in 0..12 {
        let mut score = chord_common();
        let mut doc = chord_document();
        for id in ["n0", "n0_1"] {
            common_event(&mut score, id)
                .written
                .as_mut()
                .unwrap()
                .time_modification = Some(TimeModification {
                actual: 3,
                normal: 2,
                normal_type: Some(NoteType::N16),
                normal_dots: 1,
            });
            let e = native_event(&mut doc, id);
            e.attributes
                .insert("tm".into(), Scalar::String("3:2".into()));
            e.attributes
                .insert("tm_normal_type".into(), Scalar::String("16".into()));
            e.attributes
                .insert("tm_normal_dots".into(), Scalar::Integer(1));
        }
        let member = common_event(&mut score, "n0_1");
        let native = native_event(&mut doc, "n0_1");
        match mutation {
            0 => {
                member.written = None;
                native.attributes.remove("w");
            }
            1 => {
                member.duration = q("1/8");
                native.duration = Duration::Measured(q("1/8"));
            }
            2 => {
                member.duration = q("0");
                native.duration = Duration::Measured(q("0"));
            }
            3 => {
                member.written.as_mut().unwrap().note_type = NoteType::N32;
                native
                    .attributes
                    .insert("w".into(), Scalar::String("32".into()));
            }
            4 => {
                member.written.as_mut().unwrap().dots = 1;
                native.attributes.insert("dots".into(), Scalar::Integer(1));
            }
            5 => {
                member.written.as_mut().unwrap().size = Some(NoteSize::Cue);
                native
                    .attributes
                    .insert("note_size".into(), Scalar::String("cue".into()));
            }
            6 => {
                member.written.as_mut().unwrap().time_modification = None;
                for key in ["tm", "tm_normal_type", "tm_normal_dots"] {
                    native.attributes.remove(key);
                }
            }
            7 => {
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .actual = 6;
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .normal = 4;
                native
                    .attributes
                    .insert("tm".into(), Scalar::String("6:4".into()));
            }
            8 => {
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .normal_type = None;
                native.attributes.remove("tm_normal_type");
            }
            9 => {
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .normal_type = Some(NoteType::Eighth);
                native
                    .attributes
                    .insert("tm_normal_type".into(), Scalar::String("8".into()));
            }
            10 => {
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .normal_dots = 2;
                native
                    .attributes
                    .insert("tm_normal_dots".into(), Scalar::Integer(2));
            }
            11 => {
                member
                    .written
                    .as_mut()
                    .unwrap()
                    .time_modification
                    .as_mut()
                    .unwrap()
                    .actual = 0;
                native
                    .attributes
                    .insert("tm".into(), Scalar::String("0:2".into()));
            }
            _ => unreachable!(),
        }
        assert!(
            common_error(&score, "beam.chord_rhythm"),
            "common mutation {mutation}"
        );
        assert!(
            native_error(&doc, "beam.chord_rhythm"),
            "native mutation {mutation}"
        );
    }
    // Equal flag count alone is insufficient: all members need written spelling.
    let mut score = chord_common();
    for id in ["n0", "n0_1"] {
        common_event(&mut score, id).written = None;
    }
    assert!(common_error(&score, "beam.chord_rhythm"));
}

#[test]
fn native_chord_rhythm_normalization_is_semantic_and_malformed_fields_never_panic() {
    let mut doc = chord_document();
    native_event(&mut doc, "n0")
        .attributes
        .insert("tm".into(), Scalar::String("3:2".into()));
    native_event(&mut doc, "n0_1")
        .attributes
        .insert("tm".into(), Scalar::String("03:02".into()));
    native_event(&mut doc, "n0_1")
        .attributes
        .insert("dots".into(), Scalar::Integer(0));
    // Whole-native validation still rejects invalid lexical attributes. The
    // topology comparator must not confuse their integer value with raw text.
    assert!(!native_error(&doc, "beam.chord_rhythm"));
    assert!(validate(&doc, None)
        .iter()
        .any(|d| d.severity == Severity::Error));
    for (key, value) in [
        ("tm", Scalar::String("3:broken".into())),
        ("tm", Scalar::String("3:2:1".into())),
        ("tm", Scalar::Boolean(true)),
        ("w", Scalar::Integer(16)),
        ("dots", Scalar::Integer(-1)),
        ("note_size", Scalar::String("unknown".into())),
        ("tm_normal_type", Scalar::String("unknown".into())),
        ("tm_normal_dots", Scalar::Integer(-1)),
    ] {
        let mut doc = doc.clone();
        native_event(&mut doc, "n0_1")
            .attributes
            .insert(key.into(), value);
        assert!(native_error(&doc, "beam.chord_rhythm"), "{key}");
    }
}

#[test]
fn chord_stems_are_authored_equal_up_down_or_all_absent() {
    for (head, member) in [
        (None, Some(StemDirection::Up)),
        (Some(StemDirection::Down), None),
        (Some(StemDirection::Up), Some(StemDirection::Down)),
        (Some(StemDirection::None), Some(StemDirection::None)),
        (Some(StemDirection::Double), Some(StemDirection::Double)),
    ] {
        let mut score = chord_common();
        let mut doc = chord_document();
        for (id, stem) in [("n0", head), ("n0_1", member)] {
            common_event(&mut score, id).stem = stem;
            if let Some(stem) = stem {
                native_event(&mut doc, id)
                    .attributes
                    .insert("stem".into(), Scalar::String(stem.name().into()));
            }
        }
        assert!(common_error(&score, "beam.chord_stem"));
        assert!(native_error(&doc, "beam.chord_stem"));
    }
    let mut doc = chord_document();
    native_event(&mut doc, "n0_1")
        .attributes
        .insert("stem".into(), Scalar::Integer(1));
    assert!(native_error(&doc, "beam.chord_stem"));
}

#[test]
fn chord_groups_reject_reused_ambiguous_and_nonconsecutive_members() {
    for mutation in 0..5 {
        let mut score = chord_common();
        let mut doc = chord_document();
        match mutation {
            0 => {
                common_event(&mut score, "n1").chord = Some("chord_n0".into());
                native_event(&mut doc, "n1")
                    .attributes
                    .insert("chord".into(), Scalar::String("chord_n0".into()));
            }
            1 => {
                common_event(&mut score, "n0_1").id = EventId("n0".into());
                native_event(&mut doc, "n0_1")
                    .attributes
                    .insert("id".into(), Scalar::String("n0".into()));
                // An ambiguous head is already rejected by beam.event. Put an
                // authored level on the other member of a separate triad too.
                common_event(&mut score, "n1_2").id = EventId("n1_1".into());
                native_event(&mut doc, "n1_2")
                    .attributes
                    .insert("id".into(), Scalar::String("n1_1".into()));
            }
            2 => {
                let mut unrelated = common_event(&mut score, "n0").clone();
                unrelated.id = EventId("other".into());
                unrelated.chord = None;
                score.parts[0].measures[0].events.insert(1, unrelated);
                let mut unrelated = native_event(&mut doc, "n0").clone();
                unrelated
                    .attributes
                    .insert("id".into(), Scalar::String("other".into()));
                unrelated.attributes.remove("chord");
                doc.frames[0].records.insert(1, Record::Event(unrelated));
            }
            3 => {
                common_event(&mut score, "n0_1").offset = q("1/8");
                let member = doc.frames[0].records.remove(1);
                doc.frames.insert(
                    1,
                    Frame {
                        time: q("1/8"),
                        records: vec![member],
                        source: source(),
                    },
                );
            }
            4 => {
                score.parts[0].staves = 2;
                common_event(&mut score, "n0_1").staff = 2;
                doc.parts[0].staves = 2;
                native_event(&mut doc, "n0_1")
                    .attributes
                    .insert("staff".into(), Scalar::Integer(2));
            }
            _ => unreachable!(),
        }
        assert!(
            common_error(&score, "beam.chord_group"),
            "common mutation {mutation}"
        );
        assert!(
            native_error(&doc, "beam.chord_group"),
            "native mutation {mutation}"
        );
    }
}

#[test]
fn chord_members_must_be_ordinary_measured_pitched_notes() {
    for pitch in [Pitch::Rest, Pitch::Spacer, Pitch::Unpitched("snare".into())] {
        let mut score = chord_common();
        common_event(&mut score, "n0_1").pitch = pitch.clone();
        assert!(common_error(&score, "beam.chord_profile"));
        let mut doc = chord_document();
        native_event(&mut doc, "n0_1").pitch = pitch;
        assert!(native_error(&doc, "beam.chord_profile"));
    }
    let mut doc = chord_document();
    native_event(&mut doc, "n0_1")
        .attributes
        .insert("kind".into(), Scalar::String("cue".into()));
    assert!(native_error(&doc, "beam.chord_profile"));
    let mut doc = chord_document();
    native_event(&mut doc, "n0_1").duration = Duration::Grace;
    assert!(native_error(&doc, "beam.chord_profile"));
    let mut score = chord_common();
    common_event(&mut score, "n0_1").measure_rest = Some(true);
    assert!(common_error(&score, "beam.chord_profile"));
}

#[test]
fn unmarked_or_invalid_chord_heads_still_interrupt_runs() {
    for invalid in [false, true] {
        let mut score = chord_common();
        let mut doc = chord_document();
        score.beams.retain(|b| b.event.0 != "n1");
        doc.frames[1]
            .records
            .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
        if invalid {
            common_event(&mut score, "n1_1").duration = q("1/8");
            native_event(&mut doc, "n1_1").duration = Duration::Measured(q("1/8"));
        }
        assert!(common_error(&score, "beam.interrupted"));
        assert!(native_error(&doc, "beam.interrupted"));
    }
}

#[test]
fn validated_chords_never_hide_unrelated_overlaps_in_either_direction() {
    for at in ["0", "1/8", "7/8"] {
        for first in [true, false] {
            let mut score = chord_common();
            let mut other = common_event(&mut score, "n0").clone();
            other.id = EventId("other".into());
            other.chord = None;
            other.offset = q(at);
            other.duration = q("1/8");
            let index = if first {
                0
            } else {
                score.parts[0].measures[0].events.len()
            };
            score.parts[0].measures[0].events.insert(index, other);
            assert!(common_error(&score, "beam.overlap"));
            let mut doc = chord_document();
            let mut other = native_event(&mut doc, "n0").clone();
            other
                .attributes
                .insert("id".into(), Scalar::String("other".into()));
            other.attributes.remove("chord");
            other.duration = Duration::Measured(q("1/8"));
            if at == "0" {
                let index = if first {
                    0
                } else {
                    doc.frames[0].records.len()
                };
                doc.frames[0].records.insert(index, Record::Event(other));
            } else {
                doc.frames.push(Frame {
                    time: q(at),
                    records: vec![Record::Event(other)],
                    source: source(),
                });
                doc.frames.sort_by(|a, b| a.time.cmp(&b.time));
            }
            assert!(native_error(&doc, "beam.overlap"));
        }
    }
    // An earlier, unmarked chord is also an overlap obstacle when a later run
    // opens before its independent notes finish sounding.
    let mut score = chord_common();
    score.beams.retain(|b| b.event.0 != "n0");
    score
        .beams
        .iter_mut()
        .filter(|b| b.event.0 == "n1")
        .for_each(|b| b.value = BeamValue::Begin);
    for id in ["n0", "n0_1"] {
        common_event(&mut score, id).duration = q("1/2");
    }
    assert!(common_error(&score, "beam.overlap"));
    let mut doc = chord_document();
    doc.frames[0]
        .records
        .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
    for number in [1, 2] {
        mark(&mut doc, 1, number)
            .attributes
            .insert("value".into(), Scalar::String("begin".into()));
    }
    for id in ["n0", "n0_1"] {
        native_event(&mut doc, id).duration = Duration::Measured(q("1/2"));
    }
    assert!(native_error(&doc, "beam.overlap"));
}

#[test]
fn chord_heads_do_not_expand_the_beamed_grace_principal_profile() {
    let mut doc = graces();
    add_native_chord(&mut doc, "principal", 2);
    assert!(native_error(&doc, "beam.grace"));
    let mut score = chord_common();
    score.beams.clear();
    for (index, value) in [BeamValue::Begin, BeamValue::End].into_iter().enumerate() {
        let id = format!("g{index}");
        score.beams.push(beam(&id, None, value));
        score.graces.push(ScoreGraceNote {
            visible: None,
            notehead: None,
            attack_dynamics: None,
            id: EventId(id),
            anchor: EventId("n0".into()),
            order: index as i64 - 2,
            pitch: "D4".parse().unwrap(),
            written: Some(WrittenDuration {
                note_type: NoteType::Eighth,
                size: None,
                dots: 0,
                time_modification: None,
            }),
            slash: None,
            stem: None,
            accidental: None,
            timing: GraceTiming::default(),
        });
    }
    assert!(common_error(&score, "beam.grace"));
    let mut doc = graces();
    add_native_chord(&mut doc, "g0", 2);
    assert!(native_error(&doc, "beam.chord_profile"));
}

#[test]
fn native_chord_ownership_uses_state_before_all_events_at_the_onset() {
    let mut doc = chord_document();
    doc.parts[0].staves = 2;
    // Place state after the members and marks. Their inherited staff is still 2.
    doc.frames[0].records.push(Record::State(State {
        target: "v".into(),
        attributes: [("staff".into(), Scalar::Integer(2))].into(),
        source: source(),
    }));
    assert_valid(&doc);
    // Explicit staff 1 on just one member must disagree with inherited staff 2.
    native_event(&mut doc, "n0_1")
        .attributes
        .insert("staff".into(), Scalar::Integer(1));
    assert!(native_error(&doc, "beam.chord_group"));
}

#[test]
fn chord_validation_preserves_member_only_ties_accidentals_and_every_playback_event() {
    use scorestream_core::{
        notation::{perform_document, PerformanceLimits, RelationId},
        score::{AccidentalGlyph, PrintedAccidental, ScoreTie},
    };
    let mut score = chord_common();
    common_event(&mut score, "n0_1").accidental = Some(PrintedAccidental {
        glyph: AccidentalGlyph::Natural,
        cautionary: Some(true),
        parentheses: Some(false),
    });
    score.ties.push(ScoreTie {
        id: RelationId("tie_member".into()),
        events: vec![EventId("n0_1".into()), EventId("n1_1".into())],
    });
    let before = score.clone();
    assert!(validate_score(&score).is_empty());
    assert_eq!(score, before);
    assert_eq!(score.parts[0].measures[0].events.len(), 9);
    assert!(common_event(&mut score, "n0").accidental.is_none());
    let mut doc = chord_document();
    doc.frames[0].records.push(Record::Mark(Mark {
        target: "#n0_1".into(),
        kind: "accidental".into(),
        attributes: [
            ("value".into(), Scalar::String("natural".into())),
            ("cautionary".into(), Scalar::Boolean(true)),
            ("parentheses".into(), Scalar::Boolean(false)),
        ]
        .into(),
        source: source(),
    }));
    for (frame, phase, anchor) in [
        (0, RelationPhase::Start, "#n0_1"),
        (1, RelationPhase::End, "#n1_1"),
    ] {
        doc.frames[frame].records.push(Record::Relation(Relation {
            kind: "tie".into(),
            id: "tie_member".into(),
            phase,
            anchor: anchor.into(),
            attributes: Attributes::new(),
            source: source(),
        }));
    }
    let before = doc.clone();
    assert_valid(&doc);
    assert_eq!(doc, before);
    assert!(perform_document(&doc, PerformanceLimits::default())
        .unwrap_err()
        .iter()
        .any(|d| d.code == "navigation.performance_capability"));
    // Playback remains a separately qualified projection. Add its authored
    // capability and boundary only after checking beam metadata preservation.
    doc.required_extensions
        .push(scorestream_core::navigation_extension::EXTENSION.into());
    doc.frames.push(Frame {
        time: doc.end.clone(),
        records: vec![Record::Mark(Mark {
            target: "p".into(),
            kind: scorestream_core::navigation_extension::KIND.into(),
            attributes: [
                ("measure".into(), Scalar::String("m".into())),
                ("side".into(), Scalar::String("right".into())),
                ("style".into(), Scalar::String("regular".into())),
            ]
            .into(),
            source: source(),
        })],
        source: source(),
    });
    assert_valid(&doc);
    let performed = perform_document(&doc, PerformanceLimits::default()).unwrap();
    assert_eq!(performed.events.len(), 9);
    assert!(performed.events.iter().any(|e| e.written_id.0 == "n0_1"));
    assert!(performed.events.iter().any(|e| e.written_id.0 == "n1_2"));
    doc.required_extensions.retain(|e| e != EXTENSION);
    for frame in &mut doc.frames {
        frame
            .records
            .retain(|r| !matches!(r, Record::Mark(m) if m.kind == KIND));
    }
    assert_eq!(
        performed,
        perform_document(&doc, PerformanceLimits::default()).unwrap()
    );
}
