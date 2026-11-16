//! Validated source-route metadata only. This type contains no resolved/playable
//! score and cannot authorize audio or select any musical interpretation.
use super::{EventId, PartId};
use crate::{
    Diagnostic, Document, Duration, Rational, Record, Scalar, Severity, SourceRange,
    navigation as nav, navigation_extension as ext, repeat_traversal as route,
};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
pub struct TraceFermataOccurrence {
    pub event: EventId,
    pub written_event: EventId,
    pub part: PartId,
    pub measure_occurrence: usize,
    pub start: Rational,
    pub end: Rational,
}
#[derive(Clone, Debug, Serialize)]
pub struct PlaybackRouteTrace {
    pub written_events: usize,
    pub written_graces: usize,
    pub authored_fermatas: usize,
    pub measure_visits: usize,
    pub event_occurrences: usize,
    pub end_quarter: Rational,
    pub fermatas: Vec<TraceFermataOccurrence>,
    pub passive_capabilities: Vec<String>,
    pub required_sound_review: Vec<String>,
    pub navigation_disclosures: Vec<String>,
    pub scope: String,
}
fn error(code: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(code, message, SourceRange::default())]
}
/// Trace ordinary repeat/ending visits on the complete unchanged validated score.
/// No hold duration, note gate, gain curve, instrument patch or audible event is inferred.
pub fn trace_document_route(doc: &Document) -> Result<PlaybackRouteTrace, Vec<Diagnostic>> {
    if doc.document_kind != crate::DocumentKind::Score {
        return Err(error(
            "trace.document_kind",
            "Route trace requires a complete score document",
        ));
    }
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut parts: Vec<_> = doc
        .parts
        .iter()
        .map(|p| nav::Part {
            id: PartId(p.id.clone()),
            measures: vec![],
        })
        .collect();
    let indexes: BTreeMap<_, _> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.0.clone(), i))
        .collect();
    let mut bars = vec![];
    for f in &doc.frames {
        for r in &f.records {
            match r {
                Record::Measure(m) => {
                    let i = *indexes
                        .get(&m.target)
                        .ok_or_else(|| error("trace.measure", "Unknown measure part"))?;
                    parts[i].measures.push(nav::Measure {
                        id: m.id.clone(),
                        label: m
                            .attributes
                            .get("label")
                            .and_then(Scalar::as_str)
                            .unwrap_or(&m.id)
                            .into(),
                        start: f.time.clone(),
                        length: m.length.clone(),
                    });
                }
                Record::Mark(m) if m.kind == ext::KIND => {
                    bars.push(ext::decode(m).map_err(|e| vec![e])?)
                }
                _ => (),
            }
        }
    }
    for p in &parts {
        let mut at = Rational::zero();
        for m in &p.measures {
            if m.start != at || !m.length.is_positive() {
                return Err(error(
                    "trace.measure_coverage",
                    "Trace requires contiguous positive measures from zero",
                ));
            }
            at = &at + &m.length;
        }
        if at != doc.end {
            return Err(error(
                "trace.measure_coverage",
                "Measures must cover written end exactly",
            ));
        }
    }
    let navigation =
        nav::reconcile(&parts, &bars, "document").map_err(|e| error(&e.code, e.message))?;
    let visits = route::traverse(&navigation.movement, route::Policy::default())
        .map_err(|e| error("trace.navigation", format!("Closed route rejected: {e:?}")))?;
    let measures = &parts[0].measures;
    let voices: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.as_str(), v.part.as_str()))
        .collect();
    let mut events = BTreeMap::new();
    let mut event_counts = vec![0usize; measures.len()];
    let mut grace_count = 0;
    for (at, e) in doc.events() {
        let id = e
            .id()
            .ok_or_else(|| error("trace.identity", "Every event requires a stable identity"))?;
        let i = measures
            .partition_point(|m| m.start <= *at)
            .checked_sub(1)
            .ok_or_else(|| error("trace.event", "Event precedes first measure"))?;
        if at >= &(&measures[i].start + &measures[i].length) {
            return Err(error("trace.event", "Event is outside a half-open measure"));
        }
        let duration = match &e.duration {
            Duration::Measured(r) => r.clone(),
            Duration::Grace => {
                grace_count += 1;
                Rational::zero()
            }
        };
        let end = at + &duration;
        if end > &measures[i].start + &measures[i].length {
            return Err(error(
                "trace.event",
                "Written event crosses a measure boundary",
            ));
        }
        let part = voices
            .get(e.voice.as_str())
            .ok_or_else(|| error("trace.voice", "Unknown event voice"))?;
        events.insert(id, (i, at.clone(), end, PartId((*part).into())));
        event_counts[i] += 1;
    }
    let mut signs = vec![vec![]; measures.len()];
    let mut authored = 0;
    for f in &doc.frames {
        for r in &f.records {
            if let Record::Mark(m) = r {
                if m.kind == crate::fermata_extension::KIND {
                    let sign = crate::fermata_extension::decode(m).map_err(|e| vec![e])?;
                    let (i, start, end, part) = events
                        .get(sign.event.0.as_str())
                        .ok_or_else(|| error("trace.fermata", "Unknown fermata anchor"))?;
                    signs[*i].push((sign.event, start.clone(), end.clone(), part.clone()));
                    authored += 1;
                }
            }
        }
    }
    let mut result=PlaybackRouteTrace {written_events:events.len(),written_graces:grace_count,authored_fermatas:authored,measure_visits:0,event_occurrences:0,end_quarter:Rational::zero(),fermatas:vec![],passive_capabilities:doc.required_extensions.iter().filter(|e|super::passive::is_passive(e)).cloned().collect(),required_sound_review:doc.required_extensions.iter().filter(|e|!super::passive::is_passive(e)).cloned().collect(),navigation_disclosures:visits.diagnostics.iter().map(|d|format!("{d:?}")).collect(),scope:"Route/anchor trace only. No sound interpretation, hold seconds, policy qualification, tie-jump realization or renderability is certified.".into()};
    for step in visits.steps {
        if let route::Step::ReplayMeasure {
            measure_index: i,
            occurrence_index,
            ..
        } = step
        {
            let m = &measures[i];
            for (id, start, end, part) in &signs[i] {
                result.fermatas.push(TraceFermataOccurrence {
                    event: EventId(format!("occ{occurrence_index}:{}", id.0)),
                    written_event: id.clone(),
                    part: part.clone(),
                    measure_occurrence: occurrence_index,
                    start: &result.end_quarter + &(start - &m.start),
                    end: &result.end_quarter + &(end - &m.start),
                });
            }
            result.measure_visits += 1;
            result.event_occurrences = result
                .event_occurrences
                .checked_add(event_counts[i])
                .ok_or_else(|| error("trace.count", "Event occurrence count overflow"))?;
            result.end_quarter = &result.end_quarter + &m.length;
        }
    }
    Ok(result)
}
