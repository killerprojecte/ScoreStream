//! Conversion-to-clock contract only. The performed snapshot below is an
//! explicit fixture, not a bypass of the still-unsupported source projector.
use scorestream_core::{
    Duration, Pitch, Rational, SourceRange,
    fermata_timing::{
        ExplicitFermataPolicy, ExplicitHoldGroup, SeekPosition, TimingLimits, interpret,
    },
    notation::*,
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn id(s: &str) -> EventId {
    EventId(s.into())
}
fn origin(s: &str) -> Origin {
    Origin {
        object_id: s.into(),
        source: SourceRange::default(),
    }
}

#[test]
fn imported_authored_anchors_join_an_explicit_snapshot_without_enabling_audio() {
    let model = MusicXml
        .read(include_str!("fixtures/fermata-note-rest.musicxml"))
        .unwrap()
        .value;
    let sfs = SfsAdapter.write(&model).unwrap().value;
    let source = scorestream_codec::parse(&sfs).unwrap();
    let part = &model.parts[0];
    let measure = &part.measures[0];
    let events: Vec<_> = measure
        .events
        .iter()
        .map(|e| ResolvedEvent {
            id: id(&format!("fixture:{}", e.id)),
            voice: e.voice.clone(),
            part: part.id.clone(),
            staff: StaffId {
                part: part.id.clone(),
                number: 1,
            },
            instrument: part.instrument.clone().unwrap(),
            start: e.offset.clone(),
            duration: Duration::Measured(e.duration.clone()),
            written_pitch: e.pitch.clone(),
            // The synthetic fixture contains only concert G4 notes and a rest.
            sounding_pitch: (e.pitch != Pitch::Rest).then(|| r("67")),
            role: EventRole::Note,
            grace: None,
            origin: origin(&e.id.0),
        })
        .collect();
    let performed = PerformedScore {
        events: events
            .iter()
            .zip(&measure.events)
            .map(|(e, written)| EventOccurrence {
                id: e.id.clone(),
                written_id: written.id.clone(),
                measure_occurrence: 0,
            })
            .collect(),
        measures: vec![MeasureOccurrence {
            written_id: measure.id.clone(),
            part: part.id.clone(),
            label: measure.label.clone(),
            occurrence_index: 0,
            start: r("0"),
            length: r("4"),
        }],
        score: ResolvedScore {
            end: r("4"),
            relations: vec![ResolvedRelation {
                id: model.ties[0].id.clone(),
                scope: Scope::Voice(events[0].voice.clone()),
                kind: RelationKind::Tie {
                    sound: true,
                    print: true,
                },
                anchors: events[..2]
                    .iter()
                    .map(|e| RelationAnchor {
                        event: e.id.clone(),
                        at: e.start.clone(),
                        pedal_value: None,
                        origin: e.origin.clone(),
                    })
                    .collect(),
                origin: origin("fixture:tie"),
            }],
            events,
            directions: vec![],
            tempos: vec![],
        },
        diagnostics: vec![],
    };
    let policy = ExplicitFermataPolicy {
        groups: vec![
            ExplicitHoldGroup {
                id: "tie-tail".into(),
                members: vec![id("fixture:stop")],
                added_seconds: r("1/3"),
            },
            ExplicitHoldGroup {
                id: "rest".into(),
                members: vec![id("fixture:rest")],
                added_seconds: r("2/3"),
            },
        ],
    };
    let before_source = source.clone();
    let before_performed = performed.clone();
    let result = interpret(&source, &performed, &policy, TimingLimits::default()).unwrap();
    assert_eq!(source, before_source);
    assert_eq!(performed, before_performed);
    assert_eq!(
        result
            .authored
            .iter()
            .map(|a| a.sign.clone())
            .collect::<Vec<_>>(),
        model.fermatas
    );
    assert_eq!(result.duration_seconds(), &r("3"));
    assert_eq!(result.events[1].written_id, id("stop"));
    assert_eq!(result.events[1].written_quarter_start, r("1"));
    assert_eq!(result.events[1].written_quarter_end, r("2"));
    assert_eq!(result.events[1].seconds_end, r("4/3"));
    assert_eq!(result.events[2].seconds_start, r("4/3"));
    assert_eq!(result.events[2].seconds_end, r("3"));
    assert_eq!(
        result.seek(&r("7/6")),
        SeekPosition::Hold {
            group_id: "tie-tail".into(),
            boundary: r("2"),
            elapsed_seconds: r("1/6"),
        }
    );
    assert_eq!(
        result.seek(&r("3")),
        SeekPosition::Music { quarter: r("4") }
    );
    // No extension, source mark or tie was removed to reach the clock fixture.
    // All normal source-to-playback entry points remain fail-closed.
    for errors in [
        lower_document(&source).unwrap_err(),
        perform_document(&source, PerformanceLimits::default()).unwrap_err(),
    ] {
        assert!(
            errors
                .iter()
                .any(|d| d.code == "fermata.performance_unsupported")
        );
    }
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, model);
}
