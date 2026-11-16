use scorestream_core::{
    notation::{
        lower_document, perform_document, EventId, PartId, PerformanceLimits, Placement,
        RelationId, RelationKind, VoiceId,
    },
    score::{
        GraceTiming, ScoreDocument, ScoreEvent, ScoreGraceNote, ScoreMeasure, ScorePart, ScoreSlur,
        ScoreVoice, SlurEndpoint, SlurOrientation,
    },
    slur_extension::{decode_endpoint, encode_endpoint, validate_score, EXTENSION},
    validate, Document, DocumentKind, Duration, Event, Frame, Part, Pitch, Rational, Record,
    Relation, RelationPhase, Scalar, Severity, SourceRange, Voice,
};
use serde_json::json;

fn q(value: &str) -> Rational {
    value.parse().unwrap()
}
fn endpoint(id: &str) -> SlurEndpoint {
    SlurEndpoint {
        event: EventId(id.into()),
        bezier_x: None,
        bezier_y: None,
        orientation: None,
        placement: None,
    }
}
fn slur(id: &str, start: &str, end: &str) -> ScoreSlur {
    ScoreSlur {
        id: RelationId(id.into()),
        start: endpoint(start),
        end: endpoint(end),
    }
}
fn document() -> Document {
    let mut doc = Document::new(DocumentKind::Score);
    doc.required_extensions.push(EXTENSION.into());
    doc.parts.push(Part {
        id: "p".into(),
        instrument: "piano".into(),
        staves: 2,
        display_name: None,
        source: SourceRange::default(),
    });
    doc.voices.push(Voice {
        id: "v".into(),
        part: "p".into(),
        staff: 1,
        source: SourceRange::default(),
    });
    doc.end = q("2");
    for (id, at, phase) in [
        ("a", "0", RelationPhase::Start),
        ("b", "1", RelationPhase::End),
    ] {
        doc.frames.push(Frame {
            time: q(at),
            records: vec![
                Record::Event(Event {
                    voice: "v".into(),
                    pitch: "C4".parse().unwrap(),
                    duration: Duration::Measured(q("1")),
                    attributes: [("id".into(), Scalar::String(id.into()))].into(),
                    source: SourceRange::default(),
                }),
                Record::Relation(encode_endpoint("s".into(), id.into(), phase, &endpoint(id))),
            ],
            source: SourceRange::default(),
        });
    }
    doc
}
fn common() -> ScoreDocument {
    ScoreDocument {
        slurs: vec![slur("s", "a", "b")],
        parts: vec![ScorePart {labels:None,
            id: PartId("p".into()),
            name: None,
            instrument: None,
            instrument_metadata: None,
            staves: 2,
            voices: vec![ScoreVoice {
                id: VoiceId("v".into()),
                staff: 1,
            }],
            measures: vec![ScoreMeasure {
                id: "m".into(),
                label: "1".into(),
                start: q("0"),
                length: q("2"),
                implicit: false,
                states: vec![],
                events: [("a", "0"), ("b", "1")]
                    .into_iter()
                    .map(|(id, at)| ScoreEvent {
                        visible: None,
                        notehead: None,
                        attack_dynamics: None,
                        id: EventId(id.into()),
                        voice: VoiceId("v".into()),
                        staff: 1,
                        offset: q(at),
                        duration: q("1"),
                        pitch: "C4".parse().unwrap(),
                        chord: None,
                        written: None,
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
fn relation(doc: &mut Document, frame: usize) -> &mut Relation {
    doc.frames[frame]
        .records
        .iter_mut()
        .find_map(|r| match r {
            Record::Relation(r) => Some(r),
            _ => None,
        })
        .unwrap()
}
fn event(doc: &mut Document, frame: usize) -> &mut Event {
    doc.frames[frame]
        .records
        .iter_mut()
        .find_map(|r| match r {
            Record::Event(e) => Some(e),
            _ => None,
        })
        .unwrap()
}
fn assert_valid(doc: &Document) {
    let diagnostics = validate(doc, None);
    assert!(
        diagnostics.iter().all(|d| d.severity != Severity::Error),
        "{diagnostics:?}"
    );
}
fn assert_error(doc: &Document, code: &str) {
    let diagnostics = validate(doc, None);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.code == code),
        "{diagnostics:?}"
    );
}
fn assert_common_error(score: &ScoreDocument, code: &str) {
    let diagnostics = validate_score(score);
    assert!(
        diagnostics.iter().any(|d| d.code == code),
        "{diagnostics:?}"
    );
}
fn add_graces(doc: &mut Document) {
    for (id, order) in [("z_first", -2), ("a_later", -1)] {
        doc.frames[0].records.push(Record::Event(Event {
            voice: "v".into(),
            pitch: "D4".parse().unwrap(),
            duration: Duration::Grace,
            attributes: [
                ("id".into(), Scalar::String(id.into())),
                ("kind".into(), Scalar::String("grace".into())),
                ("anchor".into(), Scalar::String("#a".into())),
                ("order".into(), Scalar::Integer(order)),
            ]
            .into(),
            source: SourceRange::default(),
        }));
    }
}
fn grace_slur(doc: &mut Document, start: &str, end: &str) {
    for frame in &mut doc.frames {
        frame.records.retain(|r| !matches!(r, Record::Relation(_)));
    }
    doc.frames[0].records.extend([
        Record::Relation(encode_endpoint(
            "s".into(),
            start.into(),
            RelationPhase::Start,
            &endpoint(start),
        )),
        Record::Relation(encode_endpoint(
            "s".into(),
            end.into(),
            RelationPhase::End,
            &endpoint(end),
        )),
    ]);
}

#[test]
fn exact_endpoint_codec_preserves_independent_optional_fields() {
    for phase in [RelationPhase::Start, RelationPhase::End] {
        for (x, y) in [
            (None, None),
            (Some("-789/13"), None),
            (None, Some("0")),
            (Some("1/7"), Some("-19/11")),
        ] {
            for orientation in [
                None,
                Some(SlurOrientation::Over),
                Some(SlurOrientation::Under),
            ] {
                for placement in [None, Some(Placement::Above), Some(Placement::Below)] {
                    let original = SlurEndpoint {
                        event: EventId("n".into()),
                        bezier_x: x.map(q),
                        bezier_y: y.map(q),
                        orientation,
                        placement,
                    };
                    let encoded = encode_endpoint("r".into(), "n".into(), phase, &original);
                    assert_eq!(encoded.id, "r");
                    assert_eq!(encoded.phase, phase);
                    assert_eq!(encoded.attributes.contains_key("bezier_x"), x.is_some());
                    assert_eq!(encoded.attributes.contains_key("bezier_y"), y.is_some());
                    assert_eq!(decode_endpoint(&encoded).unwrap(), original);
                }
            }
        }
    }
}

#[test]
fn independent_start_end_geometry_and_placement_are_valid_only_with_capability() {
    let mut doc = document();
    relation(&mut doc, 0).attributes = [
        ("bezier_x".into(), Scalar::String("13/2".into())),
        ("orientation".into(), Scalar::String("over".into())),
        ("placement".into(), Scalar::String("below".into())),
    ]
    .into();
    relation(&mut doc, 1).attributes = [
        ("bezier_y".into(), Scalar::String("-7/3".into())),
        ("orientation".into(), Scalar::String("under".into())),
        ("placement".into(), Scalar::String("above".into())),
    ]
    .into();
    assert_valid(&doc);
    let decoded: Document = serde_json::from_value(serde_json::to_value(&doc).unwrap()).unwrap();
    assert_valid(&decoded);
    doc.required_extensions.clear();
    assert_error(&doc, "slur.requirement");
    assert_error(&doc, "relation_attribute_conflict");
}

#[test]
fn legacy_slurs_keep_ordinary_lowering_and_immutable_placement() {
    let mut doc = document();
    doc.required_extensions.clear();
    relation(&mut doc, 0).attributes = [
        ("line_type".into(), Scalar::String("dashed".into())),
        ("placement".into(), Scalar::String("above".into())),
    ]
    .into();
    assert_valid(&doc);
    let score = lower_document(&doc).unwrap();
    assert_eq!(score.relations.len(), 1);
    assert!(matches!(score.relations[0].kind, RelationKind::Slur));
    relation(&mut doc, 1)
        .attributes
        .insert("placement".into(), Scalar::String("below".into()));
    assert_error(&doc, "relation_attribute_conflict");
}

#[test]
fn authored_fidelity_never_enters_ordinary_performance() {
    let doc = document();
    assert_valid(&doc);
    assert!(lower_document(&doc)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "slur.performance_unsupported"));
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
}

#[test]
fn requirement_must_be_supported_nonempty_and_complete() {
    let mut doc = document();
    doc.required_extensions[0] = "scorestream.slur@2".into();
    assert_error(&doc, "extension_unsupported");
    doc.required_extensions = vec![EXTENSION.into()];
    doc.document_kind = DocumentKind::Fragment;
    assert_error(&doc, "slur.document");
    doc.document_kind = DocumentKind::Score;
    for frame in &mut doc.frames {
        frame.records.retain(|r| !matches!(r, Record::Relation(_)));
    }
    assert_error(&doc, "slur.empty_requirement");
}

#[test]
fn malformed_endpoint_fields_reject_direct_ast_and_json() {
    for (key, value, code) in [
        ("bezier_x", Scalar::Integer(2), "slur.coordinate"),
        ("bezier_y", Scalar::String("1/0".into()), "slur.coordinate"),
        (
            "orientation",
            Scalar::String("above".into()),
            "slur.orientation",
        ),
        ("placement", Scalar::String("over".into()), "slur.placement"),
        ("line_type", Scalar::String("solid".into()), "slur.field"),
        ("bezier_x2", Scalar::String("2".into()), "slur.field"),
        ("number", Scalar::Integer(1), "slur.field"),
        ("source_xml", Scalar::String("payload".into()), "slur.field"),
    ] {
        let mut doc = document();
        relation(&mut doc, 0).attributes.insert(key.into(), value);
        assert_error(&doc, code);
        let wire = serde_json::to_value(&doc).unwrap();
        assert_error(&serde_json::from_value(wire).unwrap(), code);
    }
    for anchor in ["a", "##a", "#bad id", "#", "#missing"] {
        let mut doc = document();
        relation(&mut doc, 0).anchor = anchor.into();
        assert_error(&doc, "slur.event");
    }
}

#[test]
fn exactly_two_distinct_endpoints_per_unique_relation_id() {
    let mut doc = document();
    let extra = relation(&mut doc, 0).clone();
    doc.frames[0].records.push(Record::Relation(extra));
    assert_error(&doc, "slur.phase");
    let mut doc = document();
    relation(&mut doc, 1).phase = RelationPhase::Continue;
    assert_error(&doc, "slur.phase");
    let mut doc = document();
    relation(&mut doc, 1).anchor = "#a".into();
    assert_error(&doc, "slur.anchor");
    assert_error(&doc, "slur.onset");
    let mut doc = document();
    relation(&mut doc, 0).id = "bad id".into();
    assert_error(&doc, "slur.id");
}

#[test]
fn duplicate_parallel_slurs_keep_distinct_relation_identities() {
    let mut doc = document();
    for frame in 0..2 {
        let mut other = relation(&mut doc, frame).clone();
        other.id = "parallel".into();
        doc.frames[frame].records.push(Record::Relation(other));
    }
    assert_valid(&doc);
    let mut score = common();
    score.slurs.push(slur("parallel", "a", "b"));
    assert!(validate_score(&score).is_empty());
    score.slurs[1].id = score.slurs[0].id.clone();
    assert_common_error(&score, "slur.id");
}

#[test]
fn resting_unpitched_and_silent_cue_anchors_reject() {
    for pitch in [Pitch::Rest, Pitch::Unpitched("snare".into())] {
        let mut doc = document();
        event(&mut doc, 0).pitch = pitch;
        assert_error(&doc, "slur.anchor");
    }
    for kind in ["cue", "spacer"] {
        let mut doc = document();
        event(&mut doc, 0)
            .attributes
            .insert("kind".into(), Scalar::String(kind.into()));
        assert_error(&doc, "slur.anchor");
    }
    let mut score = common();
    score.parts[0].measures[0].events[0].pitch = Pitch::Rest;
    assert_common_error(&score, "slur.anchor");
}

#[test]
fn same_part_cross_voice_and_staff_allowed_but_cross_part_rejected() {
    let mut doc = document();
    doc.voices.push(Voice {
        id: "other".into(),
        part: "p".into(),
        staff: 2,
        source: SourceRange::default(),
    });
    event(&mut doc, 1).voice = "other".into();
    assert_valid(&doc);
    doc.parts.push(Part {
        id: "q".into(),
        instrument: "piano".into(),
        staves: 2,
        display_name: None,
        source: SourceRange::default(),
    });
    doc.voices[1].part = "q".into();
    assert_error(&doc, "slur.part");
}

#[test]
fn measured_endpoints_require_increasing_onsets() {
    let mut doc = document();
    relation(&mut doc, 0).phase = RelationPhase::End;
    relation(&mut doc, 1).phase = RelationPhase::Start;
    assert_error(&doc, "slur.order");
    let mut doc = document();
    let last = doc.frames.pop().unwrap();
    doc.frames[0].records.extend(last.records);
    assert_error(&doc, "slur.order");
    let mut score = common();
    score.parts[0].measures[0].events[1].offset = q("0");
    assert_common_error(&score, "slur.order");
}

#[test]
fn same_onset_graces_use_authored_order_and_principal_not_id_order() {
    for (start, end) in [("z_first", "a_later"), ("z_first", "a"), ("a_later", "a")] {
        let mut doc = document();
        add_graces(&mut doc);
        grace_slur(&mut doc, start, end);
        assert_valid(&doc);
        doc.frames[0].records.reverse();
        assert_valid(&doc);
    }
    for (start, end) in [("a_later", "z_first"), ("a", "z_first")] {
        let mut doc = document();
        add_graces(&mut doc);
        grace_slur(&mut doc, start, end);
        assert_error(&doc, "slur.order");
    }
}

#[test]
fn fake_grace_orders_and_unrelated_principals_do_not_authorize_equal_onsets() {
    let mut doc = document();
    add_graces(&mut doc);
    grace_slur(&mut doc, "z_first", "a");
    for record in &mut doc.frames[0].records {
        if let Record::Event(e) = record {
            if e.id() == Some("z_first") {
                e.duration = Duration::Measured(q("1"));
            }
        }
    }
    assert_error(&doc, "slur.anchor");
    assert_error(&doc, "slur.order");
    let mut doc = document();
    add_graces(&mut doc);
    grace_slur(&mut doc, "z_first", "a_later");
    for record in &mut doc.frames[0].records {
        if let Record::Event(e) = record {
            if e.id() == Some("a_later") {
                e.attributes
                    .insert("anchor".into(), Scalar::String("#b".into()));
            }
        }
    }
    assert_error(&doc, "slur.anchor");
    assert_error(&doc, "slur.order");
}

#[test]
fn common_model_validates_grace_identity_order_and_ownership() {
    let mut score = common();
    for (id, order) in [("z_first", -2), ("a_later", -1)] {
        score.graces.push(ScoreGraceNote {
            visible: None,
            notehead: None,
            attack_dynamics: None,
            id: EventId(id.into()),
            anchor: EventId("a".into()),
            order,
            pitch: "D4".parse().unwrap(),
            written: None,
            slash: None,
            stem: None,
            accidental: None,
            timing: GraceTiming::default(),
        });
    }
    score.slurs = vec![slur("s", "z_first", "a_later"), slur("t", "a_later", "a")];
    assert!(validate_score(&score).is_empty());
    score.graces[0].order = -1;
    assert_common_error(&score, "slur.anchor");
    score.graces[0].order = -2;
    score.graces[0].anchor = EventId("missing".into());
    assert_common_error(&score, "slur.anchor");
}

#[test]
fn ambiguous_event_identities_and_common_unknown_fields_fail_closed() {
    let mut doc = document();
    let duplicate = event(&mut doc, 0).clone();
    doc.frames[0].records.push(Record::Event(duplicate));
    assert_error(&doc, "slur.event");
    let mut score = common();
    let duplicate = score.parts[0].measures[0].events[0].clone();
    score.parts[0].measures[0].events.push(duplicate);
    assert_common_error(&score, "slur.event");
    let mut value = serde_json::to_value(common()).unwrap();
    value["slurs"][0]["start"]["default_x"] = json!("1");
    assert!(serde_json::from_value::<ScoreDocument>(value).is_err());
    let mut value = serde_json::to_value(common()).unwrap();
    value["slurs"][0]["start"]["bezier_x"] = json!(1);
    assert!(serde_json::from_value::<ScoreDocument>(value).is_err());
    let empty = serde_json::to_value(ScoreDocument::default()).unwrap();
    assert!(empty.get("slurs").is_none());
    let decoded: ScoreDocument = serde_json::from_value(empty).unwrap();
    assert!(decoded.slurs.is_empty());
}

#[test]
fn common_foreign_ids_remain_opaque_until_native_normalization() {
    let mut score = common();
    score.parts[0].id = PartId("part from XML #1".into());
    score.parts[0].voices[0].id = VoiceId("voice from XML #1".into());
    for event in &mut score.parts[0].measures[0].events {
        event.voice = VoiceId("voice from XML #1".into());
    }
    score.parts[0].measures[0].events[0].id = EventId("first note #1".into());
    score.parts[0].measures[0].events[1].id = EventId("last note #2".into());
    score.slurs = vec![slur("slur from XML #1", "first note #1", "last note #2")];
    assert!(validate_score(&score).is_empty());
    score.slurs[0].id = RelationId(String::new());
    assert_common_error(&score, "slur.id");
}

#[test]
fn common_tie_and_slur_cannot_reuse_one_relation_identity() {
    let mut score = common();
    score.ties.push(scorestream_core::score::ScoreTie {
        id: RelationId("s".into()),
        events: vec![EventId("a".into()), EventId("b".into())],
    });
    assert_common_error(&score, "slur.id");
    score.ties[0].id = RelationId("separate tie identity".into());
    assert!(validate_score(&score).is_empty());
}
