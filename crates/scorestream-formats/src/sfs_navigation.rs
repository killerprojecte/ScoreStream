//! Lossless authored navigation bridge; no performed event expansion.
use crate::{Format, MappingEffect, MappingReport, navigation};
use scorestream_core::{
    Attributes, Mark, Rational, Record, Scalar, SourceRange, navigation_extension as ext,
    repeat_traversal as route, score::*,
};
use std::collections::BTreeMap;
pub(crate) fn check_profile(score: &ScoreDocument, report: &mut MappingReport) {
    if score.barlines.is_empty() {
        return;
    }
    let navigation = match navigation::from_score(score, "sfs_document") {
        Ok(n) => n,
        Err(e) => {
            report.append(e);
            return;
        }
    };
    let checked = match route::validate(&navigation.movement, route::Policy::default()) {
        Ok(v) => v,
        Err(e) => {
            report.push(Format::Sfs,MappingEffect::Unsupported,"navigation.route_policy","/barlines",format!("SFS navigation requires the supported closed ordinary-repeat profile: {e:?}. No endpoint or continuation policy is inferred."));
            return;
        }
    };
    for diagnostic in checked.diagnostics {
        let (feature, message) = match diagnostic {
            route::Diagnostic::ConventionalTwoPasses { .. } => (
                "navigation.conventional_two_passes",
                "Missing authored count retains the reported two-pass performance convention; no count is inserted into the score",
            ),
            route::Diagnostic::ImplicitStartAtMovementStart { .. } => (
                "navigation.implicit_start",
                "Backward repeat has no authored forward mark; route policy starts at movement entry without adding a sign",
            ),
            route::Diagnostic::UnclosedFinalEndingContinues { .. } => {
                unreachable!("Strict conversion never enables final-ending continuation")
            }
        };
        report.push(
            Format::Sfs,
            MappingEffect::Normalized,
            feature,
            "/barlines",
            message,
        );
    }
    report.push(Format::Sfs,MappingEffect::Normalized,"navigation.authored_only","/barlines","Authored boundaries roundtrip through the required navigation capability; repeat engraving is unavailable and performed playback requires the separate supported closed-route projection");
}
pub(crate) fn read_mark(mark: &Mark, score: &mut ScoreDocument, report: &mut MappingReport) {
    match ext::decode(mark) {
        Ok(b) => score.barlines.push(b),
        Err(e) => report.push_source(
            Format::Sfs,
            MappingEffect::Invalid,
            e.code,
            &mark.target,
            e.message,
            Some(e.source),
        ),
    }
}
pub(crate) fn sort_barlines(score: &mut ScoreDocument) {
    let indices: BTreeMap<_, _> = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(pi, p)| {
            p.measures
                .iter()
                .enumerate()
                .map(move |(mi, m)| ((&p.id, m.id.as_str()), (pi, mi)))
        })
        .collect();
    score.barlines.sort_by_key(|b| {
        (
            indices.get(&(&b.part, b.measure.as_str())).copied(),
            b.location,
        )
    });
}
pub(crate) fn write_barline(
    b: &ScoreBarline,
    part: String,
    measure: String,
    at: Rational,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    let mut a = Attributes::new();
    let mut text = |key: &str, value: String| {
        a.insert(key.into(), Scalar::String(value));
    };
    text("measure", measure);
    text(
        "side",
        if b.location == BarlineLocation::Left {
            "left"
        } else {
            "right"
        }
        .into(),
    );
    if let Some(style) = b.style {
        text("style", ext::style_name(style).into());
    }
    if let Some(id) = &b.id {
        text("source_id", id.clone());
    }
    if let Some(r) = &b.repeat {
        text(
            "repeat",
            if r.direction == RepeatDirection::Forward {
                "forward"
            } else {
                "backward"
            }
            .into(),
        );
    }
    if let Some(e) = &b.ending {
        text(
            "ending",
            match e.phase {
                EndingPhase::Start => "start",
                EndingPhase::Stop => "stop",
                EndingPhase::Discontinue => "discontinue",
            }
            .into(),
        );
        text(
            "passes",
            e.passes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        if !e.text.is_empty() {
            text("ending_text", e.text.clone());
        }
        if let Some(y) = &e.default_y {
            text("ending_y", y.to_string());
        }
    }
    if let Some(times) = b.repeat.as_ref().and_then(|r| r.times) {
        a.insert("times".into(), Scalar::Integer(i64::from(times)));
    }
    frames.entry(at).or_default().push(Record::Mark(Mark {
        target: part,
        kind: ext::KIND.into(),
        attributes: a,
        source: SourceRange::default(),
    }));
}
