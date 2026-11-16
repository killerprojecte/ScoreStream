//! Closed, non-playable discovery for an explicitly disclosed practice route.
//! No constructor here returns a ResolvedScore or authorizes sound rendering.
use super::*;
use crate::{Diagnostic, Document, Rational, Record, Severity, instrument_extension};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug)]
pub enum PracticeDiscoveryPolicy {
    WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
}
#[derive(Clone, Debug)]
pub struct PracticeMidiSelection {
    pub part: String,
    pub instrument: String,
    pub bank: u16,
    pub program_zero_based: u8,
}
#[derive(Clone, Debug)]
pub struct PracticeJointDiscovery {
    source: Document,
    discovery: CombinedPerformanceDiscovery,
}
impl PracticeJointDiscovery {
    pub fn source(&self) -> &Document {
        &self.source
    }
    pub fn discovery(&self) -> &CombinedPerformanceDiscovery {
        &self.discovery
    }
    pub fn nonrealizations(&self) -> [&'static str; 4] {
        [
            "wedge: no authored crescendo/diminuendo trajectory",
            "dashes: no inferred acoustic continuation or termination",
            "ornaments: no auxiliary pitches, repetitions or ornament timing",
            "slur: no legato overlap, phrasing or bowing interpretation",
        ]
    }
}
/// Source-qualified discovery after checking the same initial GM/centered MIDI
/// subset used by the native local-mixer receipt. This checks intent, not bank
/// bytes, CC quantization or synthesis. Native callers MUST also match their
/// sealed mixer receipt to the exact source and keep it for eventual rendering.
pub fn discover_document_practice_joint(
    doc: &Document,
    _policy: PracticeDiscoveryPolicy,
    articulation: ExplicitArticulationPolicy,
    routes: &[PracticeMidiSelection],
) -> Result<PracticeJointDiscovery, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let fail = |message: &str| {
        vec![Diagnostic::error(
            "practice.midi_selection",
            message,
            doc.source,
        )]
    };
    if routes.len() != doc.parts.len() {
        return Err(fail("Require exact selected route for every source part"));
    }
    let mut seen = BTreeSet::new();
    for route in routes {
        let part = doc
            .parts
            .iter()
            .find(|p| p.id == route.part)
            .ok_or_else(|| fail("Unknown selected part"))?;
        if !seen.insert(&route.part)
            || part.instrument != route.instrument
            || route.bank != 0
            || route.program_zero_based > 127
        {
            return Err(fail("Duplicate or mismatched explicit bank-0 route"));
        }
        let records: Vec<_> = doc
            .frames
            .iter()
            .flat_map(|f| f.records.iter().map(move |r| (f, r)))
            .filter_map(|(f, r)| match r {
                Record::Mark(m)
                    if m.kind == instrument_extension::KIND && m.target == route.part =>
                {
                    Some((f, m))
                }
                _ => None,
            })
            .collect();
        if records.len() != 1 || records[0].0.time != Rational::zero() {
            return Err(fail("Require one initial authored instrument per part"));
        }
        let value = instrument_extension::decode(records[0].1).map_err(|d| vec![d])?;
        if value
            .midi_device
            .as_ref()
            .is_some_and(|d| !d.name.is_empty())
        {
            return Err(fail("Named external devices are not local PCM routes"));
        }
        let midi = value
            .midi_instrument
            .as_ref()
            .ok_or_else(|| fail("Require authored MIDI assignment"))?;
        if midi.program.map(u16::from) != Some(u16::from(route.program_zero_based) + 1)
            || midi.pan.as_ref().is_some_and(|p| *p != Rational::zero())
        {
            return Err(fail(
                "Source program must match explicit preset; only centered authored pan supported",
            ));
        }
    }
    let discovery = super::combined::discover_practice(doc, articulation)?;
    Ok(PracticeJointDiscovery {
        source: doc.clone(),
        discovery,
    })
}
