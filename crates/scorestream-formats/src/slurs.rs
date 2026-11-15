//! Authored slur graph and endpoint data, not an implicit legato realization.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{Severity, score::ScoreDocument, slur_extension};

pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    for d in slur_extension::validate_score(score) {
        report.push_source(
            Format::ScoreModel,
            if d.severity == Severity::Error {
                MappingEffect::Invalid
            } else {
                MappingEffect::Preserved
            },
            d.code,
            d.object_id
                .map(|id| format!("/slurs/{id}"))
                .unwrap_or_else(|| "/slurs".into()),
            d.message,
            Some(d.source),
        );
    }
}

pub(crate) fn authored_only(score: &ScoreDocument, format: Format, report: &mut MappingReport) {
    if !score.slurs.is_empty() {
        report.push(format, MappingEffect::Preserved, "slur.authored_only", "/slurs",
            "Independent slur identities, exact event anchors and per-endpoint Bézier coordinates, orientation and placement are retained. No engraving or legato interpretation is supplied; authored slurs require an explicit future realization before playback.");
    }
}

pub(crate) fn sort(score: &mut ScoreDocument) {
    score.slurs.sort_by(|a, b| a.id.cmp(&b.id));
}
