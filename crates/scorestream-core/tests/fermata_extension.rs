use scorestream_core::{
    Attributes, Diagnostic, Document, DocumentKind, Duration, Event, Frame, Mark, Part, Pitch,
    Rational, Record, Scalar, Severity, SourceRange, Voice,
    fermata_extension::{EXTENSION, KIND, decode, orientation_name, shape_name},
    notation::{EventId, lower_document},
    score::{FermataOrientation, FermataShape, ScoreDocument, ScoreFermata},
    validate,
};
use serde_json::json;

fn mark() -> Mark {
    Mark {
        target: "#n1".into(),
        kind: KIND.into(),
        attributes: Attributes::new(),
        source: SourceRange::line(7, 30),
    }
}

fn document() -> Document {
    let mut doc = Document::new(DocumentKind::Score);
    doc.required_extensions.push(EXTENSION.into());
    doc.parts.push(Part {
        id: "p1".into(),
        instrument: "piano".into(),
        staves: 1,
        display_name: None,
        source: SourceRange::default(),
    });
    doc.voices.push(Voice {
        id: "v1".into(),
        part: "p1".into(),
        staff: 1,
        source: SourceRange::default(),
    });
    doc.end = Rational::from_integer(4);
    doc.frames.push(Frame {
        time: Rational::one(),
        records: vec![
            Record::Event(Event {
                voice: "v1".into(),
                pitch: "C4".parse().unwrap(),
                duration: Duration::Measured(Rational::one()),
                attributes: [("id".into(), Scalar::String("n1".into()))].into(),
                source: SourceRange::default(),
            }),
            Record::Mark(mark()),
        ],
        source: SourceRange::default(),
    });
    doc
}

fn has_error(doc: &Document, code: &str) -> bool {
    validate(doc, None)
        .iter()
        .any(|d| d.severity == Severity::Error && d.code == code)
}

fn assert_valid(doc: &Document) -> Vec<Diagnostic> {
    let diagnostics = validate(doc, None);
    assert!(
        diagnostics.iter().all(|d| d.severity != Severity::Error),
        "{diagnostics:?}"
    );
    diagnostics
}

#[test]
fn decoder_preserves_absence_and_every_explicit_shape_orientation() {
    let absent = decode(&mark()).unwrap();
    assert_eq!(absent.event, EventId("n1".into()));
    assert_eq!(absent.shape, None);
    assert_eq!(absent.orientation, None);
    assert_eq!(absent.default_y, None);
    assert_eq!(absent.relative_y, None);
    for shape in [
        FermataShape::Normal,
        FermataShape::Angled,
        FermataShape::Square,
        FermataShape::DoubleAngled,
        FermataShape::DoubleSquare,
        FermataShape::DoubleDot,
        FermataShape::HalfCurve,
        FermataShape::Curlew,
    ] {
        for orientation in [FermataOrientation::Upright, FermataOrientation::Inverted] {
            let mut m = mark();
            m.attributes = [
                ("shape".into(), Scalar::String(shape_name(shape).into())),
                (
                    "orientation".into(),
                    Scalar::String(orientation_name(orientation).into()),
                ),
                ("default_y".into(), Scalar::String("-123/7".into())),
                ("relative_y".into(), Scalar::String("19/3".into())),
            ]
            .into();
            let decoded = decode(&m).unwrap();
            assert_eq!(decoded.shape, Some(shape));
            assert_eq!(decoded.orientation, Some(orientation));
            assert_eq!(decoded.default_y, Some("-123/7".parse().unwrap()));
            assert_eq!(decoded.relative_y, Some("19/3".parse().unwrap()));
            let encoded = serde_json::to_value(&decoded).unwrap();
            assert_eq!(encoded["shape"], shape_name(shape));
            assert_eq!(encoded["orientation"], orientation_name(orientation));
            assert_eq!(
                serde_json::from_value::<ScoreFermata>(encoded).unwrap(),
                decoded
            );
        }
    }
}

#[test]
fn decoder_rejects_unregistered_fields_types_values_and_scopes() {
    for field in [
        "id",
        "default_x",
        "relative_x",
        "font_size",
        "sound",
        "hold",
    ] {
        let mut m = mark();
        m.attributes
            .insert(field.into(), Scalar::String("1".into()));
        let diagnostic = decode(&m).unwrap_err();
        assert_eq!(diagnostic.code, "fermata.field", "{field}");
        assert_eq!(diagnostic.source, m.source);
    }
    for field in ["shape", "orientation", "default_y", "relative_y"] {
        for value in [Scalar::Integer(0), Scalar::Boolean(false)] {
            let mut m = mark();
            m.attributes.insert(field.into(), value);
            assert_eq!(decode(&m).unwrap_err().code, "fermata.field", "{field}");
        }
    }
    for (field, values, code) in [
        ("shape", vec!["", "long", "Normal"], "fermata.shape"),
        (
            "orientation",
            vec!["", "above", "Upright"],
            "fermata.orientation",
        ),
        (
            "default_y",
            vec!["1.5", "+1", "-0", "2/4", "1/0", "1/-2"],
            "fermata.placement",
        ),
        (
            "relative_y",
            vec!["1.5", "+1", "-0", "2/4", "1/0", "1/-2"],
            "fermata.placement",
        ),
    ] {
        for value in values {
            let mut m = mark();
            m.attributes
                .insert(field.into(), Scalar::String(value.into()));
            assert_eq!(decode(&m).unwrap_err().code, code, "{field}={value}");
        }
    }
    for target in ["score", "p1", "p1.s1", "v1", "n1", "#", "##n1", "#bad id"] {
        let mut m = mark();
        m.target = target.into();
        assert_eq!(decode(&m).unwrap_err().code, "fermata.event", "{target}");
    }
    let mut m = mark();
    m.kind = "fermata".into();
    assert_eq!(decode(&m).unwrap_err().code, "fermata.kind");
}

#[test]
fn typed_model_is_strict_and_old_documents_remain_unchanged() {
    let old = json!({ "metadata": {}, "parts": [] });
    let doc: ScoreDocument = serde_json::from_value(old.clone()).unwrap();
    assert!(doc.fermatas.is_empty());
    assert_eq!(serde_json::to_value(doc).unwrap(), old);
    let minimal = json!({ "event": "n1" });
    let sign: ScoreFermata = serde_json::from_value(minimal.clone()).unwrap();
    assert_eq!(sign, decode(&mark()).unwrap());
    for (field, value) in [
        ("id", json!("f1")),
        ("hold", json!("2")),
        ("shape", json!("unknown")),
        ("orientation", json!("above")),
        ("default_y", json!(1)),
        ("relative_y", json!(1.5)),
    ] {
        let mut invalid = minimal.clone();
        invalid[field] = value;
        assert!(serde_json::from_value::<ScoreFermata>(invalid).is_err());
    }
    assert!(serde_json::from_value::<ScoreFermata>(json!({})).is_err());
}

#[test]
fn registered_capability_preserves_each_distinct_note_or_rest_and_warns() {
    let mut doc = document();
    for (id, pitch) in [
        ("rest", Pitch::Rest),
        ("drum", Pitch::Unpitched("snare".into())),
    ] {
        doc.frames[0].records.push(Record::Event(Event {
            voice: "v1".into(),
            pitch,
            duration: Duration::Measured(Rational::one()),
            attributes: [("id".into(), Scalar::String(id.into()))].into(),
            source: SourceRange::default(),
        }));
        let mut m = mark();
        m.target = format!("#{id}");
        doc.frames[0].records.push(Record::Mark(m));
    }
    let diagnostics = assert_valid(&doc);
    assert!(diagnostics.iter().any(|d| {
        d.code == "fermata.realization_unavailable" && d.severity == Severity::Warning
    }));
    let signs: Vec<_> = doc.frames[0]
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Mark(m) => Some(decode(m).unwrap().event),
            _ => None,
        })
        .collect();
    assert_eq!(
        signs,
        [
            EventId("n1".into()),
            EventId("rest".into()),
            EventId("drum".into())
        ]
    );
}

#[test]
fn validation_rejects_missing_unknown_empty_and_fragment_capabilities() {
    let mut doc = document();
    doc.required_extensions.clear();
    assert!(has_error(&doc, "fermata.requirement"));
    doc.required_extensions.push("scorestream.fermata@2".into());
    assert!(has_error(&doc, "extension_unsupported"));
    assert!(has_error(&doc, "fermata.requirement"));
    doc.required_extensions = vec![EXTENSION.into()];
    doc.frames[0]
        .records
        .retain(|r| !matches!(r, Record::Mark(_)));
    assert!(has_error(&doc, "fermata.empty_requirement"));
    let mut fragment = document();
    fragment.document_kind = DocumentKind::Fragment;
    fragment.span = Some(fragment.end.clone());
    assert!(has_error(&fragment, "fragment_global_state"));
    fragment.required_extensions.clear();
    assert!(has_error(&fragment, "fragment_global_state"));
}

#[test]
fn validation_rejects_unresolved_scope_grace_spacer_and_duplicate_anchors() {
    for target in ["score", "p1", "p1.s1", "v1", "#missing"] {
        let mut doc = document();
        let Record::Mark(m) = &mut doc.frames[0].records[1] else {
            unreachable!()
        };
        m.target = target.into();
        assert!(has_error(&doc, "fermata.event"), "{target}");
    }
    let mut doc = document();
    let Record::Event(event) = &mut doc.frames[0].records[0] else {
        unreachable!()
    };
    event.pitch = Pitch::Spacer;
    assert!(has_error(&doc, "fermata.event"));
    let mut doc = document();
    let Record::Event(event) = &mut doc.frames[0].records[0] else {
        unreachable!()
    };
    event.duration = Duration::Grace;
    assert!(has_error(&doc, "fermata.event"));
    let mut doc = document();
    doc.frames[0].records.push(Record::Mark(mark()));
    assert!(has_error(&doc, "fermata.duplicate"));
}

#[test]
fn mark_frame_must_match_exact_event_onset_not_its_sounding_span() {
    for time in ["0", "3/2", "2"] {
        let mut doc = document();
        let record = doc.frames[0].records.remove(1);
        doc.frames.push(Frame {
            time: time.parse().unwrap(),
            records: vec![record],
            source: SourceRange::default(),
        });
        doc.frames.sort_by(|a, b| a.time.cmp(&b.time));
        assert!(has_error(&doc, "fermata.onset"), "{time}");
    }
}

#[test]
fn lowering_fails_explicitly_without_inventing_hold_duration() {
    let doc = document();
    assert_valid(&doc);
    let diagnostics = lower_document(&doc).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "fermata.performance_unsupported");
    let mut plain = doc;
    plain.required_extensions.clear();
    plain.frames[0]
        .records
        .retain(|r| !matches!(r, Record::Mark(_)));
    assert!(assert_valid(&plain).is_empty());
    let lowered = lower_document(&plain).unwrap();
    assert_eq!(lowered.events.len(), 1);
    assert_eq!(
        plain.events().next().unwrap().1.duration.measured(),
        Some(&Rational::one())
    );
}
