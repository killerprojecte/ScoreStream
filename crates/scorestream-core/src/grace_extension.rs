//! Required fidelity for imported authored grace notes. Legacy core grace is unchanged.
#![allow(clippy::result_large_err)]
use crate::{
    score::GraceTiming, Diagnostic, Document, DocumentKind, Mark, Pitch, Rational, Record, Scalar,
};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.grace@1";
pub const KIND: &str = "scorestream.grace";
fn error(mark: &Mark, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, mark.source)
}
pub fn decode(mark: &Mark) -> Result<GraceTiming, Diagnostic> {
    if mark.kind != KIND
        || mark
            .attributes
            .keys()
            .any(|k| !["steal_previous", "steal_following", "make_time"].contains(&k.as_str()))
    {
        return Err(error(
            mark,
            "grace.field",
            "Unknown authored-grace metadata field",
        ));
    }
    let number = |key| -> Result<Option<Rational>, Diagnostic> {
        match mark.attributes.get(key) {
            None => Ok(None),
            Some(Scalar::String(v)) => v
                .parse()
                .map(Some)
                .map_err(|_| error(mark, "grace.timing", "Expected an exact canonical rational")),
            _ => Err(error(
                mark,
                "grace.timing",
                "Expected an exact rational string",
            )),
        }
    };
    let timing = GraceTiming {
        steal_previous: number("steal_previous")?,
        steal_following: number("steal_following")?,
        make_time_quarters: number("make_time")?,
    };
    if !timing.valid() {
        return Err(error(
            mark,
            "grace.timing",
            "Steal percentages must be 0 through 100; make-time must be nonnegative",
        ));
    }
    Ok(timing)
}
pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let marks: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Mark(m) if m.kind == KIND => Some((&f.time, m)),
                _ => None,
            })
        })
        .collect();
    if !enabled && marks.is_empty() {
        return;
    }
    if !enabled {
        out.push(Diagnostic::error(
            "grace.requirement",
            "Authored grace metadata requires scorestream.grace@1",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        out.push(Diagnostic::error(
            "grace.document",
            "Authored grace capability requires a complete score",
            doc.source,
        ));
    }
    let events: BTreeMap<_, _> = doc
        .events()
        .filter_map(|(t, e)| e.id().map(|id| (id, (t, e))))
        .collect();
    let graces: Vec<_> = doc.events().filter(|(_, e)| e.is_grace()).collect();
    if graces.is_empty() || marks.is_empty() {
        out.push(Diagnostic::error(
            "grace.empty_requirement",
            "Authored grace capability requires grace events with one metadata record each",
            doc.source,
        ));
    }
    let mut marked = BTreeSet::new();
    for (time, m) in marks {
        if let Err(e) = decode(m) {
            out.push(e);
        }
        let id = m.target.strip_prefix('#').unwrap_or("");
        if !marked.insert(id) {
            out.push(error(
                m,
                "grace.duplicate",
                "Exactly one authored-grace metadata record per grace event",
            ));
        }
        match events.get(id) {
            Some((t, e)) if *t == time && e.is_grace() => {}
            _ => out.push(error(
                m,
                "grace.event",
                "Metadata must target a grace event in its own onset frame",
            )),
        }
    }
    let mut groups = BTreeMap::<&str, Vec<i64>>::new();
    let staff = |e: &crate::Event| {
        e.attributes
            .get("staff")
            .and_then(Scalar::as_i64)
            .or_else(|| {
                doc.voices
                    .iter()
                    .find(|v| v.id == e.voice)
                    .map(|v| i64::from(v.staff))
            })
    };
    for (_, e) in graces {
        if !e.id().is_some_and(|id| marked.contains(id)) {
            out.push(Diagnostic::error(
                "grace.metadata_missing",
                "Every grace in this fidelity profile needs its metadata record",
                e.source,
            ));
        }
        let anchor = e
            .attributes
            .get("anchor")
            .and_then(Scalar::as_str)
            .and_then(|s| s.strip_prefix('#'))
            .unwrap_or("");
        let order = e
            .attributes
            .get("order")
            .and_then(Scalar::as_i64)
            .unwrap_or(0);
        groups.entry(anchor).or_default().push(order);
        let principal = events.get(anchor).map(|(_, e)| *e);
        if !matches!(e.pitch, Pitch::Pitched { .. })
            || e.chord().is_some()
            || order >= 0
            || principal.is_none_or(|p| {
                p.is_grace()
                    || !matches!(p.pitch, Pitch::Pitched { .. })
                    || p.chord().is_some()
                    || p.attributes
                        .get("kind")
                        .and_then(Scalar::as_str)
                        .is_some_and(|k| k != "note")
                    || staff(e) != staff(p)
            })
        {
            out.push(Diagnostic::error("grace.profile","Only pitched before-principal grace groups on the principal's staff, without chords or silent cues, are mapped",e.source));
        }
    }
    for orders in groups.values_mut() {
        orders.sort();
        let n = orders.len() as i64;
        if orders.iter().enumerate().any(|(i, o)| *o != i as i64 - n) {
            out.push(Diagnostic::error(
                "grace.order",
                "Authored grace order must be the consecutive sequence -N through -1 per principal",
                doc.source,
            ));
        }
    }
    out.push(Diagnostic::warning("grace.realization_unavailable","Authored grace notation and timing intent are retained; playback requires a separately selected complete explicit grace allocation and sound policy",doc.source));
}
