//! Source-qualified fermata projection. No public ignore-fermata switch and no
//! playable projection is returned until all visited signs have explicit holds.
use super::{lower::LoweringProfile, perform, PerformanceLimits, PerformedScore};
use crate::{
    fermata_extension,
    fermata_timing::{
        self, AuthoredFermataAudit, ExplicitFermataPolicy, FermataOccurrence, TimingDiagnostic,
        TimingLimits, TimingProjection,
    },
    Diagnostic, Document, Rational, Record, Severity,
};
use std::collections::BTreeMap;

/// Policy-entry metadata only. In particular this contains no playable score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FermataDiscovery {
    pub authored: Vec<AuthoredFermataAudit>,
    pub occurrences: Vec<FermataOccurrence>,
}

/// Constructible only from unchanged validated source and a complete explicit
/// policy. Read-only access prevents callers from changing its route or clock.
#[derive(Clone, Debug)]
pub struct QualifiedFermataPerformance {
    performed: PerformedScore,
    timing: TimingProjection,
}
impl QualifiedFermataPerformance {
    pub fn performed(&self) -> &PerformedScore {
        &self.performed
    }
    pub fn timing(&self) -> &TimingProjection {
        &self.timing
    }
    pub fn into_timing(self) -> TimingProjection {
        self.timing
    }
}

fn fail(doc: &Document, code: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(code, message, doc.source)]
}

// Bound source work before validating or allocating the projected route. The
// pure clock applies its complete exact-arithmetic and policy budgets afterward.
fn prepare(
    doc: &Document,
    performance_limits: PerformanceLimits,
    limits: &TimingLimits,
) -> Result<PerformedScore, Vec<Diagnostic>> {
    let records = doc
        .frames
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.records.len()));
    if doc.frames.len() > limits.maximum_source_records
        || doc.parts.len() > limits.maximum_source_records
        || doc.voices.len() > limits.maximum_source_records
        || records.is_none_or(|n| n > limits.maximum_source_records)
        || doc.event_count() > limits.maximum_events
    {
        return Err(fail(
            doc,
            "fermata.timing_budget",
            "Qualified source budget exceeded",
        ));
    }
    let bounded = |r: &Rational| {
        r.numerator().bits() <= limits.maximum_rational_bits
            && r.denominator().bits() <= limits.maximum_rational_bits
    };
    if !bounded(&doc.end)
        || doc.frames.iter().any(|f| {
            !bounded(&f.time)
                || f.records.iter().any(|r| match r {
                    Record::Measure(m) => !bounded(&m.length),
                    Record::Event(e) => e.duration.measured().is_some_and(|r| !bounded(r)),
                    _ => false,
                })
        })
    {
        return Err(fail(
            doc,
            "fermata.rational_budget",
            "Qualified source timing exceeds rational bit budget",
        ));
    }
    // Bound explicit numeric controls before validation/projection can clone them
    // into repeated checkpoints. These are the lowerer's registered state fields,
    // not a second interpretation of arbitrary source attributes.
    let max_chars = limits
        .maximum_rational_bits
        .saturating_mul(2)
        .saturating_add(64);
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::State(state) = record {
                for key in ["tempo", "sound_dynamics", "transpose_chromatic"] {
                    if let Some(value) = state.attributes.get(key).and_then(crate::Scalar::as_str) {
                        let value = if key == "tempo" {
                            value.split_once(':').map(|(_, bpm)| bpm).unwrap_or(value)
                        } else {
                            value
                        };
                        if value.len() as u64 > max_chars
                            || value.parse::<Rational>().is_ok_and(|r| !bounded(&r))
                        {
                            return Err(fail(
                                doc,
                                "fermata.rational_budget",
                                "Qualified source numeric control exceeds rational bit budget",
                            ));
                        }
                    }
                }
            }
        }
    }
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    if doc.events().any(|(_, e)| e.id().is_none()) {
        return Err(fail(
            doc,
            "fermata.source_identity",
            "Qualified timing requires stable IDs for every source event",
        ));
    }
    if doc.events().any(|(_, e)| e.is_grace()) {
        return Err(fail(
            doc,
            "fermata.unsupported_grace",
            "Source grace timing is outside the qualified fermata profile",
        ));
    }
    // This private profile retains the unchanged source for typed sign discovery
    // and interpretation. Neither discovery nor failure exposes a naked score.
    perform::perform(
        doc,
        performance_limits,
        LoweringProfile::ExplicitFermatas,
        Some(limits.maximum_rational_bits),
        None,
    )
    .map_err(|e| vec![e])
}

/// Discover stable written/occurrence IDs and exact anchor coordinates. This
/// projects the actual route but exposes only authored signs and their visits.
pub fn discover_document_fermatas(doc: &Document) -> Result<FermataDiscovery, Vec<Diagnostic>> {
    let limits = TimingLimits::default();
    let performed = prepare(doc, PerformanceLimits::default(), &limits)?;
    discover(doc, &performed, &limits)
}
pub(super) fn discover(
    doc: &Document,
    performed: &PerformedScore,
    limits: &TimingLimits,
) -> Result<FermataDiscovery, Vec<Diagnostic>> {
    let mut authored = Vec::new();
    let mut by_written = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Mark(m) = record {
                if m.kind == fermata_extension::KIND {
                    if authored.len() >= limits.maximum_authored_fermatas {
                        return Err(fail(
                            doc,
                            "fermata.timing_budget",
                            "Authored fermata budget exceeded",
                        ));
                    }
                    let sign = fermata_extension::decode(m).map_err(|e| vec![e])?;
                    by_written.insert(sign.event.clone(), authored.len());
                    authored.push(AuthoredFermataAudit {
                        sign,
                        occurrence_ids: vec![],
                    });
                }
            }
        }
    }
    let mut written = BTreeMap::new();
    for (at, event) in doc.events() {
        let id = event.id().ok_or_else(|| {
            fail(
                doc,
                "fermata.source_identity",
                "Missing stable source event ID",
            )
        })?;
        written.insert(id, (at, event));
    }
    let events: BTreeMap<_, _> = performed.score.events.iter().map(|e| (&e.id, e)).collect();
    let mut occurrences = Vec::new();
    for receipt in &performed.events {
        if let Some(&index) = by_written.get(&receipt.written_id) {
            let event = events[&receipt.id];
            let (start, original) = written[receipt.written_id.0.as_str()];
            authored[index].occurrence_ids.push(receipt.id.clone());
            occurrences.push(FermataOccurrence {
                sign: authored[index].sign.clone(),
                authored_index: index,
                id: receipt.id.clone(),
                written_id: receipt.written_id.clone(),
                measure_occurrence: receipt.measure_occurrence,
                part: event.part.clone(),
                written_quarter_start: start.clone(),
                written_quarter_end: start
                    + original.duration.measured().ok_or_else(|| {
                        fail(
                            doc,
                            "fermata.unsupported_grace",
                            "Fermata discovery requires measured anchors",
                        )
                    })?,
                quarter_start: event.start.clone(),
                quarter_end: event.end(),
            });
        }
    }
    Ok(FermataDiscovery {
        authored,
        occurrences,
    })
}

/// Qualify the real source-to-performed route and consume every visited authored
/// sign through the exact policy clock. Does not choose an audio sustain policy.
pub fn perform_document_with_explicit_fermatas(
    doc: &Document,
    policy: &ExplicitFermataPolicy,
    performance_limits: PerformanceLimits,
    timing_limits: TimingLimits,
) -> Result<QualifiedFermataPerformance, Vec<Diagnostic>> {
    let performed = prepare(doc, performance_limits, &timing_limits)?;
    let mut timing = fermata_timing::interpret(doc, &performed, policy, timing_limits)
        .map_err(|e| fail(doc, e.code, e.message))?;
    retain_default_tempo_provenance(&performed, &mut timing);
    Ok(QualifiedFermataPerformance { performed, timing })
}

pub(super) fn retain_default_tempo_provenance(
    performed: &PerformedScore,
    timing: &mut TimingProjection,
) {
    // The route reducer inserts explicit checkpoint controls even for the default
    // tempo. Preserve that provenance, including restored default intervals.
    for (i, tempo) in performed.score.tempos.iter().enumerate() {
        if tempo.origin.object_id == "performance.default_tempo" {
            let end = performed
                .score
                .tempos
                .get(i + 1)
                .map(|t| &t.at)
                .unwrap_or(&performed.score.end);
            if end > &tempo.at {
                timing
                    .diagnostics
                    .push(TimingDiagnostic::PerformedDefaultQuarterTempo {
                        quarter_bpm: Rational::from_integer(120),
                        from_quarter: tempo.at.clone(),
                        until_quarter: end.clone(),
                    });
            }
        }
    }
}
