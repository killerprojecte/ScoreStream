use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{Pitch, Rational, score::*};
use std::collections::{BTreeMap, BTreeSet};
/// Validation for public typed input, including callers constructing it without a parser.
pub fn validate_score(score: &ScoreDocument) -> MappingReport {
    let mut report = MappingReport::default();
    for d in score
        .metadata
        .validate(scorestream_core::SourceRange::default())
    {
        if d.severity == scorestream_core::Severity::Error {
            report.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                d.code,
                "/metadata",
                d.message,
            );
        }
    }

    let mut invalid = |path: &str, feature: &str, message: &str| {
        report.push(
            Format::ScoreModel,
            MappingEffect::Invalid,
            feature,
            path,
            message,
        )
    };
    if score.parts.is_empty() {
        invalid("/", "parts", "Score requires at least one part");
    }
    if score.parts.len() > 256 {
        invalid("/", "parts.limit", "At most 256 parts");
    }
    let mut parts = BTreeSet::new();
    let mut voices = BTreeSet::new();
    let mut events = BTreeSet::new();
    let mut measures = BTreeSet::new();
    let mut instrument_sources = BTreeSet::new();
    for part in &score.parts {
        let path = format!("/parts/{}", part.id);
        if part.id.0.is_empty() || !parts.insert(&part.id) {
            invalid(&path, "part.identity", "Empty or duplicate part ID");
        }
        if let Some(metadata) = &part.instrument_metadata {
            if let Err(e) = scorestream_core::instrument_metadata::validate(metadata) {
                invalid(&path, e.code, e.message);
            }
            if !instrument_sources.insert(&metadata.source_id) {
                invalid(
                    &path,
                    "instrument.source_id",
                    "Duplicate instrument source ID",
                );
            }
            match scorestream_core::instrument_metadata::catalog_projection(metadata) {
                Some(p)
                    if part
                        .instrument
                        .as_ref()
                        .is_some_and(|id| id.0 == p.canonical_id) => {}
                Some(_) => invalid(
                    &path,
                    "instrument.catalog_mismatch",
                    "Instrument route differs from preserved metadata projection",
                ),
                None => invalid(
                    &path,
                    "instrument.catalog_unresolved",
                    "No exact supported catalog projection",
                ),
            }
        }
        if part.staves == 0 || part.staves > 64 {
            invalid(&path, "part.staves", "Staff count must be 1 through 64");
        }
        if part.voices.is_empty() || part.voices.len() > 256 {
            invalid(
                &path,
                "part.voices",
                "Part must declare 1 through 256 voices",
            );
        }
        for v in &part.voices {
            if v.id.0.is_empty() || !voices.insert(&v.id) {
                invalid(
                    &path,
                    "voice.identity",
                    "Empty or duplicate global voice ID",
                );
            }
            if v.staff == 0 || v.staff > part.staves {
                invalid(&path, "voice.staff", "Voice staff outside part");
            }
        }
        let mut end = Rational::zero();
        if part.measures.is_empty() {
            invalid(&path, "part.measures", "Part requires explicit measures");
        }
        for m in &part.measures {
            let path = format!("{path}/measures/{}", m.id);
            if m.id.is_empty() || !measures.insert(&m.id) {
                invalid(
                    &path,
                    "measure.identity",
                    "Empty or duplicate global measure ID",
                );
            }
            if m.start != end || !m.length.is_positive() {
                invalid(
                    &path,
                    "measure.boundary",
                    "Measures must be positive, contiguous and begin at zero",
                );
            }
            end = &m.start + &m.length;
            let mut staffs = BTreeSet::new();
            for s in &m.states {
                if s.clef.is_none() && s.key.is_none() && s.meter.is_none() && s.transpose.is_none()
                {
                    invalid(
                        &path,
                        "state.empty",
                        "Empty staff state has no mapped meaning",
                    );
                }
                if s.staff == 0 || s.staff > part.staves || !staffs.insert(s.staff) {
                    invalid(&path, "state.staff", "Invalid or duplicate staff state");
                }
                if s.clef.is_some_and(|c| !(1..=5).contains(&c.line)) {
                    invalid(&path, "clef.line", "Clef line must be 1 through 5");
                }
                if s.meter
                    .as_ref()
                    .is_some_and(|v| v.beats.is_empty() || v.beat_type == 0 || v.beats.contains(&0))
                {
                    invalid(
                        &path,
                        "meter",
                        "Meter requires positive beats and beat type",
                    );
                }
            }
            let mut chords: BTreeMap<&str, &ScoreEvent> = BTreeMap::new();
            for e in &m.events {
                let ep = format!("{path}/events/{}", e.id);
                if e.id.0.is_empty() || !events.insert(&e.id) {
                    invalid(&ep, "event.identity", "Empty or duplicate global event ID");
                }
                if !part.voices.iter().any(|v| v.id == e.voice) {
                    invalid(
                        &ep,
                        "event.voice",
                        "Event voice is not declared in this part",
                    );
                }
                if e.staff == 0 || e.staff > part.staves {
                    invalid(&ep, "event.staff", "Event staff outside part");
                }
                if e.offset.is_negative()
                    || !e.duration.is_positive()
                    || &e.offset + &e.duration > m.length
                {
                    invalid(
                        &ep,
                        "event.time",
                        "Event must have positive duration and remain within its measure",
                    );
                }
                if matches!(e.pitch, Pitch::Pitched { step, .. } if !('A'..='G').contains(&step)) {
                    invalid(&ep, "event.pitch", "Pitch step must be A through G");
                }
                if e.measure_rest.is_some() && !matches!(e.pitch, Pitch::Rest) {
                    invalid(
                        &ep,
                        "rest.measure_kind",
                        "Measure-rest identity requires a rest",
                    );
                }
                if e.measure_rest == Some(true)
                    && (e.offset != Rational::zero() || e.duration != m.length)
                {
                    invalid(
                        &ep,
                        "rest.measure_extent",
                        "Full-measure rest must span the exact actual measure from its start",
                    );
                }
                if let Some(w) = &e.written {
                    if w.dots > 32 {
                        invalid(&ep, "written.dots", "At most 32 augmentation dots");
                    }
                    if w.time_modification.as_ref().is_some_and(|tm| {
                        tm.actual == 0
                            || tm.normal == 0
                            || tm.normal_dots > 32
                            || (tm.normal_type.is_none() && tm.normal_dots != 0)
                    }) {
                        invalid(
                            &ep,
                            "written.time_modification",
                            "Invalid time modification",
                        );
                    }
                }
                if let Some(chord) = &e.chord {
                    if chord.is_empty() || !matches!(e.pitch, Pitch::Pitched { .. }) {
                        invalid(
                            &ep,
                            "chord.kind",
                            "Chord identity must be nonempty and contain pitched notes",
                        );
                    }
                    if let Some(head) = chords.get(chord.as_str()) {
                        if head.voice != e.voice || head.offset != e.offset {
                            invalid(
                                &ep,
                                "chord.members",
                                "Chord tones must share voice and onset",
                            );
                        }
                    } else {
                        chords.insert(chord, e);
                    }
                }
            }
        }
    }
    crate::accidentals::validate(score, &mut report);
    crate::ties::validate(score, &mut report);
    crate::tempo::validate(score, &mut report);
    crate::dynamics::validate(score, &mut report);
    crate::fermatas::validate(score, &mut report);
    crate::graces::validate(score, &mut report);
    crate::navigation::validate_barlines(score, &mut report);
    report
}
