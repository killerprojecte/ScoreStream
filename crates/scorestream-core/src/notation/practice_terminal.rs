//! Exact source-owned terminal dashes stops under the existing no-dashes-acoustics practice policy.
use super::PerformedScore;
use crate::{
    dashes::{self, DashesEndpoint, DashesType},
    Diagnostic, Document, Rational, Record,
};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PracticeTerminalDashesOccurrence {
    pub written_index: usize,
    pub source_endpoint: DashesEndpoint,
    pub measure_occurrence: usize,
    pub boundary: Rational,
}
pub(super) fn endpoints(doc: &Document) -> Result<Vec<(usize, DashesEndpoint)>, Diagnostic> {
    if !doc
        .required_extensions
        .iter()
        .any(|e| e == dashes::EXTENSION)
    {
        return Ok(vec![]);
    }
    let decoded = dashes::decode(doc).map_err(|e| e.into_iter().next().expect("decoder error"))?;
    let measures: std::collections::BTreeMap<_, _> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Measure(m) => {
                    Some(((m.target.as_str(), m.id.as_str()), (&f.time, &m.length)))
                }
                _ => None,
            })
        })
        .collect();
    let mut result = vec![];
    for (index, e) in decoded.into_iter().enumerate() {
        let (start, length) = measures
            .get(&(e.part.as_str(), e.measure.as_str()))
            .expect("decoded owner");
        if *start + &e.offset == doc.end {
            if e.kind != DashesType::Stop || &e.offset != *length || *start + *length != doc.end {
                return Err(Diagnostic::error("practice.terminal_dashes","Only an exact typed stop at its owning final measure's right boundary is admitted; no opening, moved endpoint or inferred control",doc.source));
            }
            result.push((index, e));
        }
    }
    Ok(result)
}
pub(super) fn receipts(
    doc: &Document,
    performed: &PerformedScore,
) -> Result<Vec<PracticeTerminalDashesOccurrence>, Vec<Diagnostic>> {
    let mut out = vec![];
    for (index, e) in endpoints(doc).map_err(|e| vec![e])? {
        for m in &performed.measures {
            if m.part.0 == e.part && m.written_id == e.measure {
                out.push(PracticeTerminalDashesOccurrence {
                    written_index: index,
                    source_endpoint: e.clone(),
                    measure_occurrence: m.occurrence_index,
                    boundary: &m.start + &e.offset,
                });
            }
        }
    }
    Ok(out)
}
